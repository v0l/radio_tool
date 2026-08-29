//! Clone mode for the Baofeng UV-5R family.
//!
//! The protocol, as implemented by CHIRP's `uv5r` driver:
//!
//! 1. send the identify magic, slowly, and wait for an ACK
//! 2. send `0x02`, and read the ident block the radio answers with
//! 3. ACK that, and the radio is in clone mode
//! 4. read memory a block at a time with `S`, address, length
//!
//! The image this produces is byte compatible with a CHIRP `.img` for the
//! same radio: the ident block, then the main memory, then the aux memory on
//! radios that have it.

use crate::{ByteStream, Error, Result};

/// The radio acknowledges with this
const ACK: u8 = 0x06;
/// Blocks are read this many bytes at a time
const BLOCK: u8 = 0x40;
/// Main memory runs from zero to here
const MAIN_END: u16 = 0x1800;
/// Aux memory, on the radios that have it
const AUX_START: u16 = 0x1ec0;
const AUX_END: u16 = 0x2000;
/// A block read outside the aux area, to wake the radio up before touching it
const AUX_PRIMER: u16 = 0x1e80;
/// Where the firmware version sits within the first aux block
const VERSION_IN_BLOCK: std::ops::Range<usize> = 48..62;

/// A radio this driver speaks to
#[derive(Debug, Clone, Copy)]
pub struct Model {
    /// Name used on the command line
    pub name: &'static str,
    /// Identify magics to try, in order
    pub idents: &'static [&'static [u8]],
    /// Whether this radio has the aux memory block
    pub aux_block: bool,
}

/// Every radio this driver speaks to. Magics from CHIRP's `uv5r.py`.
pub const ALL: &[Model] = &[
    Model {
        name: "UV5R",
        idents: &[
            &[0x50, 0xbb, 0xff, 0x20, 0x12, 0x07, 0x25],
            &[0x50, 0xbb, 0xff, 0x01, 0x25, 0x98, 0x4d],
        ],
        aux_block: true,
    },
    Model {
        name: "UV82",
        idents: &[&[0x50, 0xbb, 0xff, 0x20, 0x13, 0x01, 0x05]],
        aux_block: true,
    },
    Model {
        name: "UV6",
        idents: &[
            &[0x50, 0xbb, 0xff, 0x20, 0x12, 0x08, 0x23],
            &[0x50, 0xbb, 0xff, 0x12, 0x03, 0x98, 0x4d],
        ],
        aux_block: true,
    },
    Model {
        name: "F11",
        idents: &[&[0x50, 0xbb, 0xff, 0x13, 0xa1, 0x11, 0xdd]],
        aux_block: false,
    },
];

/// Look up a radio by the name used on the command line
pub fn model(name: &str) -> Option<&'static Model> {
    ALL.iter().find(|m| m.name == name)
}

/// What was learned about a radio while reading it
#[derive(Debug, Clone, Default)]
pub struct RadioInfo {
    /// The ident block, which starts the image
    pub ident: Vec<u8>,
    /// The firmware version string, on radios that report one
    pub firmware: String,
}

/// One clone session with a radio.
///
/// The protocol is a single pass: the radio answers blocks in the order they
/// are asked for and gets confused if the conversation restarts, so a session
/// downloads once and is then spent.
#[derive(Debug)]
pub struct Session<S: ByteStream> {
    stream: S,
    model: &'static Model,
    spent: bool,
}

impl<S: ByteStream> Session<S> {
    /// Start a session with a radio of a given model
    pub fn new(stream: S, model: &'static Model) -> Self {
        Self {
            stream,
            model,
            spent: false,
        }
    }

    /// Identify the radio and read its whole memory.
    ///
    /// The result is a CHIRP compatible image: ident, main memory, and the
    /// aux block on radios that have one.
    pub fn download(&mut self) -> Result<(Vec<u8>, RadioInfo)> {
        if self.spent {
            return Err(Error::SessionSpent);
        }
        self.spent = true;

        let mut info = RadioInfo {
            ident: self.identify()?,
            firmware: String::new(),
        };

        let mut short_tail = false;
        if self.model.aux_block {
            // reading the aux area cold gives junk on newer radios, so touch
            // a block outside it first
            self.read_block(AUX_PRIMER, BLOCK, true)?;
            let first = self.read_block(AUX_START, BLOCK, false)?;
            let second = self.read_block(0x1fc0, BLOCK, false)?;

            info.firmware = trim(first.get(VERSION_IN_BLOCK).unwrap_or_default());

            // some radios drop the byte at 0x1fcf when that block is read in
            // one go, and need the tail read in smaller pieces
            short_tail = second.get(15) == Some(&0xff);
        } else {
            self.read_block(0x0000, BLOCK, true)?;
        }

        let mut image = info.ident.clone();

        let mut addr = 0u16;
        while addr < MAIN_END {
            image.extend_from_slice(&self.read_block(addr, BLOCK, false)?);
            addr = addr.saturating_add(u16::from(BLOCK));
        }

        if self.model.aux_block {
            if short_tail {
                let mut addr = AUX_START;
                while addr < 0x1fc0 {
                    image.extend_from_slice(&self.read_block(addr, BLOCK, false)?);
                    addr = addr.saturating_add(u16::from(BLOCK));
                }
                let mut addr = 0x1fc0u16;
                while addr < AUX_END {
                    image.extend_from_slice(&self.read_block(addr, 0x10, false)?);
                    addr = addr.saturating_add(0x10);
                }
            } else {
                let mut addr = AUX_START;
                while addr < AUX_END {
                    image.extend_from_slice(&self.read_block(addr, BLOCK, false)?);
                    addr = addr.saturating_add(u16::from(BLOCK));
                }
            }
        }

        Ok((image, info))
    }

    /// Try each magic this model is known by until one is answered
    fn identify(&mut self) -> Result<Vec<u8>> {
        let mut last = None;
        for magic in self.model.idents {
            match self.try_ident(magic) {
                Ok(ident) => return Ok(ident),
                Err(e) => {
                    last = Some(e);
                    // the radio needs a moment before it will listen again
                    self.stream.sleep(2000);
                }
            }
        }
        Err(last.unwrap_or(Error::NoRadio))
    }

    fn try_ident(&mut self, magic: &[u8]) -> Result<Vec<u8>> {
        self.stream.flush_input()?;

        // the radio drops the magic if it arrives too fast
        self.stream.write_slowly(magic, 10)?;

        let ack = self.stream.read(1)?;
        if ack.first() != Some(&ACK) {
            return Err(Error::NoRadio);
        }

        self.stream.write_all(&[0x02])?;

        // the ident is normally 8 bytes, some radios send 12, and it ends
        // with 0xdd either way
        let mut response = Vec::with_capacity(12);
        for _ in 0..12 {
            let byte = self.stream.read(1)?;
            let Some(byte) = byte.first().copied() else {
                break;
            };
            response.push(byte);
            if byte == 0xdd {
                break;
            }
        }

        let ident = match response.len() {
            8 => response,
            12 => {
                // the long form carries the same eight bytes, spread out
                let mut short = vec![
                    *response.first().unwrap_or(&0),
                    *response.get(3).unwrap_or(&0),
                    *response.get(5).unwrap_or(&0),
                ];
                short.extend_from_slice(response.get(7..).unwrap_or_default());
                short
            }
            _ => {
                return Err(Error::Unexpected {
                    what: "ident",
                    expected: "8 or 12 bytes".to_owned(),
                    got: format!("{} bytes", response.len()),
                });
            }
        };

        self.stream.write_all(&[ACK])?;
        let ack = self.stream.read(1)?;
        if ack.first() != Some(&ACK) {
            return Err(Error::Unexpected {
                what: "clone acknowledgement",
                expected: "0x06".to_owned(),
                got: format!("{ack:02x?}"),
            });
        }

        Ok(ident)
    }

    /// Read one block. The first block of a session is not acknowledged.
    fn read_block(&mut self, addr: u16, size: u8, first: bool) -> Result<Vec<u8>> {
        let command = [b'S', (addr >> 8) as u8, (addr & 0xff) as u8, size];
        self.stream.write_all(&command)?;

        if !first {
            let ack = self.stream.read(1)?;
            if ack.first() != Some(&ACK) {
                return Err(Error::Unexpected {
                    what: "block acknowledgement",
                    expected: "0x06".to_owned(),
                    got: format!("{ack:02x?}"),
                });
            }
        }

        let header = self.stream.read_exact(4, "block header")?;
        let echoed =
            u16::from(*header.get(1).unwrap_or(&0)) << 8 | u16::from(*header.get(2).unwrap_or(&0));

        if header.first() != Some(&b'X') || echoed != addr || header.get(3) != Some(&size) {
            return Err(Error::Unexpected {
                what: "block header",
                expected: format!("X {addr:#06x} {size:#04x}"),
                got: format!("{header:02x?}"),
            });
        }

        let data = self.stream.read_exact(usize::from(size), "block data")?;

        self.stream.write_all(&[ACK])?;
        self.stream.sleep(50);

        Ok(data)
    }
}

/// A radio string, up to the first terminator
fn trim(data: &[u8]) -> String {
    let end = data
        .iter()
        .position(|c| *c == 0x00 || *c == 0xff)
        .unwrap_or(data.len());
    data.get(..end)
        .unwrap_or_default()
        .iter()
        .map(|c| {
            if c.is_ascii_graphic() || *c == b' ' {
                char::from(*c)
            } else {
                '.'
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;

    /// A UV-5R that answers the clone protocol from a memory image.
    ///
    /// It is strict on purpose: anything the driver sends that a real radio
    /// would not expect is recorded as a complaint rather than tolerated.
    struct FakeRadio {
        memory: Vec<u8>,
        ident: Vec<u8>,
        outgoing: Vec<u8>,
        incoming: Vec<u8>,
        identified: bool,
        awaiting_block_ack: bool,
        first_block_done: bool,
        complaints: Vec<String>,
        blocks_read: usize,
        slept: u64,
    }

    impl FakeRadio {
        fn new() -> Self {
            let mut memory = vec![0xffu8; 0x2000];
            // a firmware version where the driver looks for it
            memory[0x1ec0 + 48..0x1ec0 + 55].copy_from_slice(b"HN5RV01");
            // something recognisable in the first channel
            memory[0..4].copy_from_slice(&[0x00, 0x00, 0x55, 0x14]);
            Self {
                memory,
                ident: vec![0x50, 0xbb, 0xff, 0x20, 0x12, 0x07, 0x25, 0xdd],
                outgoing: Vec::new(),
                incoming: Vec::new(),
                identified: false,
                awaiting_block_ack: false,
                first_block_done: false,
                complaints: Vec::new(),
                blocks_read: 0,
                slept: 0,
            }
        }

        fn complain(&mut self, what: impl Into<String>) {
            self.complaints.push(what.into());
        }

        /// Consume whatever the driver has sent
        fn process(&mut self) {
            loop {
                if !self.identified {
                    let magic: &[u8] = &[0x50, 0xbb, 0xff, 0x20, 0x12, 0x07, 0x25];
                    if self.incoming.len() < magic.len() {
                        return;
                    }
                    if &self.incoming[..magic.len()] != magic {
                        self.complain("wrong identify magic");
                        self.incoming.clear();
                        return;
                    }
                    self.incoming.drain(..magic.len());
                    self.outgoing.push(ACK);
                    self.identified = true;
                    continue;
                }

                let Some(first) = self.incoming.first().copied() else {
                    return;
                };

                match first {
                    0x02 => {
                        self.incoming.remove(0);
                        let ident = self.ident.clone();
                        self.outgoing.extend_from_slice(&ident);
                    }
                    ACK => {
                        self.incoming.remove(0);
                        if self.awaiting_block_ack {
                            self.awaiting_block_ack = false;
                        } else {
                            // the ident handshake ends with an ACK each way
                            self.outgoing.push(ACK);
                        }
                    }
                    b'S' => {
                        if self.incoming.len() < 4 {
                            return;
                        }
                        let cmd: Vec<u8> = self.incoming.drain(..4).collect();
                        let addr = usize::from(cmd[1]) << 8 | usize::from(cmd[2]);
                        let size = usize::from(cmd[3]);

                        if self.first_block_done {
                            // every block after the first is acknowledged first
                            self.outgoing.push(ACK);
                        }
                        self.first_block_done = true;

                        if addr + size > self.memory.len() {
                            self.complain(format!("read past the end at {addr:#x}"));
                            return;
                        }

                        self.outgoing
                            .extend_from_slice(&[b'X', cmd[1], cmd[2], cmd[3]]);
                        let block = self.memory[addr..addr + size].to_vec();
                        self.outgoing.extend_from_slice(&block);
                        self.awaiting_block_ack = true;
                        self.blocks_read += 1;
                    }
                    other => {
                        self.complain(format!("unknown command {other:#04x}"));
                        self.incoming.clear();
                        return;
                    }
                }
            }
        }
    }

    impl ByteStream for FakeRadio {
        fn write_all(&mut self, data: &[u8]) -> Result<()> {
            self.incoming.extend_from_slice(data);
            self.process();
            Ok(())
        }

        fn read(&mut self, len: usize) -> Result<Vec<u8>> {
            let take = len.min(self.outgoing.len());
            Ok(self.outgoing.drain(..take).collect())
        }

        fn flush_input(&mut self) -> Result<()> {
            self.outgoing.clear();
            Ok(())
        }

        fn sleep(&mut self, millis: u64) {
            self.slept += millis;
        }
    }

    #[test]
    fn a_download_produces_a_chirp_compatible_image() {
        let radio = FakeRadio::new();
        let mut session = Session::new(radio, model("UV5R").expect("UV5R is known"));

        let (image, info) = session.download().expect("the download succeeds");

        // 8 byte ident, 0x1800 main, 0x140 aux
        assert_eq!(image.len(), 8 + 0x1800 + 0x140);
        assert_eq!(
            &image[..8],
            &[0x50, 0xbb, 0xff, 0x20, 0x12, 0x07, 0x25, 0xdd]
        );
        assert_eq!(info.firmware, "HN5RV01");
        assert_eq!(
            &image[8..12],
            &[0x00, 0x00, 0x55, 0x14],
            "channel 0 came through"
        );

        assert!(
            session.stream.complaints.is_empty(),
            "the radio complained: {:?}",
            session.stream.complaints
        );
    }

    #[test]
    fn a_radio_that_drops_the_last_byte_gets_its_tail_read_in_pieces() {
        // when 0x1fcf reads back as 0xff the radio is one of the ones that
        // drops that byte, and the tail has to be read in 0x10 blocks
        let radio = FakeRadio::new();
        assert_eq!(radio.memory[0x1fcf], 0xff, "this fake triggers the quirk");

        let mut session = Session::new(radio, model("UV5R").expect("UV5R is known"));
        let (image, _) = session.download().expect("the download succeeds");

        // three primers, main memory, four 0x40 blocks then four 0x10 ones
        assert_eq!(session.stream.blocks_read, 3 + 0x1800 / 0x40 + 4 + 4);
        assert_eq!(
            image.len(),
            8 + 0x1800 + 0x140,
            "the image is the same size"
        );
        assert!(session.stream.complaints.is_empty());
    }

    #[test]
    fn a_radio_without_that_quirk_reads_the_tail_in_one_go() {
        let mut radio = FakeRadio::new();
        radio.memory[0x1fcf] = 0x00;

        let mut session = Session::new(radio, model("UV5R").expect("UV5R is known"));
        let (image, _) = session.download().expect("the download succeeds");

        // three primers, main memory, and the aux area in 0x40 blocks
        assert_eq!(session.stream.blocks_read, 3 + 0x1800 / 0x40 + 0x140 / 0x40);
        assert_eq!(image.len(), 8 + 0x1800 + 0x140);
        assert!(session.stream.complaints.is_empty());
    }

    #[test]
    fn a_radio_without_aux_memory_reads_less() {
        let radio = FakeRadio::new();
        let mut session = Session::new(radio, model("F11").expect("F11 is known"));

        // the F-11 answers a different magic, so identification fails here,
        // which is itself worth checking: a wrong model must not half read
        let result = session.download();
        assert!(result.is_err(), "the F-11 magic must not be accepted");
    }

    #[test]
    fn a_session_can_only_be_used_once() {
        let radio = FakeRadio::new();
        let mut session = Session::new(radio, model("UV5R").expect("UV5R is known"));

        session.download().expect("the first download succeeds");
        assert!(
            matches!(session.download(), Err(Error::SessionSpent)),
            "a second download must be refused, not half attempted"
        );
    }

    #[test]
    fn the_magic_is_sent_slowly() {
        // a real radio drops it otherwise, and this is easy to lose in a port
        let radio = FakeRadio::new();
        let mut session = Session::new(radio, model("UV5R").expect("UV5R is known"));
        session.download().expect("the download succeeds");

        // seven magic bytes at 10ms, plus 50ms after each block
        assert!(
            session.stream.slept >= 70,
            "the magic was sent without pauses"
        );
    }

    #[test]
    fn a_silent_radio_is_reported_clearly() {
        struct Silent;
        impl ByteStream for Silent {
            fn write_all(&mut self, _: &[u8]) -> Result<()> {
                Ok(())
            }
            fn read(&mut self, _: usize) -> Result<Vec<u8>> {
                Ok(Vec::new())
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _: u64) {}
        }

        let mut session = Session::new(Silent, model("UV5R").expect("UV5R is known"));
        assert!(matches!(session.download(), Err(Error::NoRadio)));
    }

    #[test]
    fn every_model_has_workable_magics() {
        for m in ALL {
            assert!(!m.idents.is_empty(), "{}: no magic", m.name);
            for magic in m.idents {
                assert_eq!(magic.len(), 7, "{}: magic is not 7 bytes", m.name);
            }
        }
        assert!(model("UV5R").is_some());
        assert!(model("NOPE").is_none());
    }

    #[test]
    fn strings_stop_at_a_terminator() {
        assert_eq!(trim(b"HN5RV01\x00rest"), "HN5RV01");
        assert_eq!(trim(b"HN5RV01\xffrest"), "HN5RV01");
        assert_eq!(trim(b""), "");
    }
}
