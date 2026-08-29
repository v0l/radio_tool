//! Clone mode for the Baofeng UV-17Pro family.
//!
//! Despite the name this family includes the UV-5R Mini, which is a different
//! radio from the classic UV-5R in [`crate::uv5r`] and speaks a different
//! protocol. As implemented by CHIRP's `baofeng_uv17Pro` driver:
//!
//! 1. send the model's magic string, and wait for an ACK
//! 2. send three more magics, whose replies carry the model and firmware
//!    strings and which the radio wants read before it will do anything else
//! 3. read memory a block at a time with `R`, address, length
//!
//! Memory comes back lightly obfuscated, see [`crypt`].
//!
//! These radios speak the same protocol over a cable and over Bluetooth, so
//! this works against any [`ByteStream`]. Only a serial transport exists so
//! far; a Bluetooth one would drop straight in.

use crate::{ByteStream, Error, Result};

/// The radio acknowledges with this
const ACK: u8 = 0x06;
/// Memory is read this many bytes at a time
pub const BLOCK: u8 = 0x40;

/// Magics sent after the ident, with the number of bytes each is answered
/// with. The replies carry model and firmware strings, and the radio will not
/// talk until they have been read.
const HANDSHAKE: [(&[u8], usize); 3] = [
    (&[0x46], 16),
    (&[0x4d], 15),
    (
        &[
            0x53, 0x45, 0x4e, 0x44, 0x21, 0x05, 0x0d, 0x01, 0x01, 0x01, 0x04, 0x11, 0x08, 0x05,
            0x0d, 0x0d, 0x01, 0x11, 0x0f, 0x09, 0x12, 0x09, 0x10, 0x04, 0x00,
        ],
        1,
    ),
];

/// A contiguous run of radio memory
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// First address
    pub start: u16,
    /// How many bytes
    pub size: u16,
}

/// A radio this driver speaks to
#[derive(Debug, Clone, Copy)]
pub struct Model {
    /// Name used on the command line
    pub name: &'static str,
    /// Magic string that starts a clone session
    pub ident: &'static str,
    /// Model name CHIRP stamps on an image, which this driver appends so the
    /// file opens in CHIRP as well
    pub chirp_model: &'static str,
    /// The memory this radio holds
    pub regions: &'static [Region],
    /// Index into the obfuscation table
    pub key: u8,
}

impl Model {
    /// Total size of this radio's memory
    pub fn memory_size(&self) -> usize {
        self.regions.iter().map(|r| usize::from(r.size)).sum()
    }
}

/// Every radio this driver speaks to. Magics and memory maps from CHIRP.
pub const ALL: &[Model] = &[
    Model {
        name: "UV5RMINI",
        ident: "PROGRAMCOLORPROU",
        chirp_model: "UV-5R Mini",
        regions: &[
            Region {
                start: 0x0000,
                size: 0x8040,
            },
            Region {
                start: 0x9000,
                size: 0x0040,
            },
            Region {
                start: 0xa000,
                size: 0x01c0,
            },
        ],
        key: 1,
    },
    Model {
        name: "UV5GMINI",
        ident: "PROGRAMCOLORPROU",
        chirp_model: "UV-5G Mini",
        regions: &[
            Region {
                start: 0x0000,
                size: 0x8040,
            },
            Region {
                start: 0x9000,
                size: 0x0040,
            },
            Region {
                start: 0xa000,
                size: 0x01c0,
            },
        ],
        key: 1,
    },
    Model {
        name: "UV17PRO",
        ident: "PROGRAMBFNORMALU",
        chirp_model: "UV-17Pro",
        regions: &[
            Region {
                start: 0x0000,
                size: 0x8040,
            },
            Region {
                start: 0x9000,
                size: 0x0040,
            },
            Region {
                start: 0xa000,
                size: 0x02c0,
            },
            Region {
                start: 0xd000,
                size: 0x0040,
            },
        ],
        key: 1,
    },
    Model {
        name: "UV17PROGPS",
        ident: "PROGRAMCOLORPROU",
        chirp_model: "UV-17ProGPS",
        regions: &[
            Region {
                start: 0x0000,
                size: 0x8040,
            },
            Region {
                start: 0x9000,
                size: 0x0040,
            },
            Region {
                start: 0xa000,
                size: 0x02c0,
            },
            Region {
                start: 0xd000,
                size: 0x0040,
            },
        ],
        key: 1,
    },
];

/// Look up a radio by the name used on the command line
pub fn model(name: &str) -> Option<&'static Model> {
    ALL.iter().find(|m| m.name == name)
}

/// The obfuscation these radios apply to memory.
///
/// A conditional XOR against a four byte key, lifted from CHIRP. Spaces in
/// the key, and the trivial byte values, are passed through untouched, which
/// is what makes it conditional and what makes it its own inverse. Only key 1
/// is used by the radios here.
pub fn crypt(key_index: u8, data: &[u8]) -> Vec<u8> {
    const TABLE: [&[u8; 4]; 20] = [
        b"BHT ", b"CO 7", b"A ES", b" EIY", b"M PQ", b"XN Y", b"RVB ", b" HQP", b"W RC", b"MS N",
        b" SAT", b"K DH", b"ZO R", b"C SL", b"6RB ", b" JCG", b"PN V", b"J PK", b"EK L", b"I LZ",
    ];

    let Some(key) = TABLE.get(usize::from(key_index)) else {
        // an unknown key means no obfuscation rather than a panic
        return data.to_vec();
    };

    data.iter()
        .enumerate()
        .map(|(ix, byte)| {
            let k = key.get(ix % 4).copied().unwrap_or(b' ');
            let untouched =
                k == b' ' || *byte == 0x00 || *byte == 0xff || *byte == k || *byte == (k ^ 0xff);
            if untouched { *byte } else { *byte ^ k }
        })
        .collect()
}

/// What the radio said about itself
#[derive(Debug, Clone, Default)]
pub struct RadioInfo {
    /// The first handshake reply, which carries model and firmware strings
    pub ident: Vec<u8>,
}

impl RadioInfo {
    /// The ident as text, with anything unprintable shown as a dot
    pub fn describe(&self) -> String {
        self.ident
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
}

/// One clone session with a radio
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
    /// The image has the CHIRP model name appended, which is how CHIRP knows
    /// what it is looking at, so the result opens in CHIRP too.
    pub fn download(&mut self) -> Result<(Vec<u8>, RadioInfo)> {
        if self.spent {
            return Err(Error::SessionSpent);
        }
        self.spent = true;

        let info = self.identify()?;

        let mut image = Vec::with_capacity(self.model.memory_size());
        for region in self.model.regions {
            let mut addr = region.start;
            let end = u32::from(region.start) + u32::from(region.size);
            while u32::from(addr) < end {
                let remaining = end - u32::from(addr);
                let size = u16::from(BLOCK).min(remaining as u16) as u8;
                image.extend_from_slice(&self.read_block(addr, size)?);
                addr = addr.saturating_add(u16::from(size));
            }
        }

        image.extend_from_slice(self.model.chirp_model.as_bytes());
        Ok((image, info))
    }

    /// Write a codeplug back to the radio.
    ///
    /// `image` must hold at least the radio's memory. Anything past that is
    /// the model stamp a download adds, or CHIRP's metadata, and is not sent.
    ///
    /// This is the one operation here that changes a radio. A codeplug is not
    /// firmware, so a bad write costs the channel memory rather than the
    /// radio, but it is still worth knowing that nothing verifies afterwards:
    /// the radio acknowledges each block and that is all it says.
    pub fn upload(
        &mut self,
        image: &[u8],
        mut progress: impl FnMut(usize, usize),
    ) -> Result<RadioInfo> {
        if self.spent {
            return Err(Error::SessionSpent);
        }
        self.spent = true;

        let total = self.model.memory_size();
        if image.len() < total {
            return Err(Error::Unexpected {
                what: "codeplug size",
                expected: format!("at least {total} bytes for a {}", self.model.name),
                got: format!("{} bytes", image.len()),
            });
        }

        let info = self.identify()?;

        let mut done = 0usize;
        progress(0, total);

        for region in self.model.regions {
            let mut addr = region.start;
            let end = u32::from(region.start) + u32::from(region.size);

            while u32::from(addr) < end {
                // the last block of a region can be short, and the frame
                // carries its own length, so send a short block rather than
                // running past the end of the region
                let remaining = end - u32::from(addr);
                let size = u16::from(BLOCK).min(remaining as u16) as usize;

                let block = image.get(done..done + size).ok_or(Error::Unexpected {
                    what: "codeplug size",
                    expected: format!("{total} bytes"),
                    got: format!("{} bytes", image.len()),
                })?;

                self.write_block(addr, block)?;

                done += size;
                addr = addr.saturating_add(size as u16);
                progress(done, total);
            }
        }

        Ok(info)
    }

    fn write_block(&mut self, addr: u16, data: &[u8]) -> Result<()> {
        let size = u8::try_from(data.len())
            .map_err(|_| Error::Port("block longer than a frame can carry".to_owned()))?;

        let mut frame = vec![b'W', (addr >> 8) as u8, (addr & 0xff) as u8, size];
        frame.extend_from_slice(&crypt(self.model.key, data));

        self.stream.write_all(&frame)?;

        let ack = self.stream.read_exact(1, "acknowledgement of a block")?;
        if ack.first() != Some(&ACK) {
            return Err(Error::Unexpected {
                what: "acknowledgement of a block",
                expected: "0x06".to_owned(),
                got: format!("{ack:02x?} for the block at {addr:#06x}"),
            });
        }
        Ok(())
    }

    fn identify(&mut self) -> Result<RadioInfo> {
        self.stream.flush_input()?;

        self.stream.write_all(self.model.ident.as_bytes())?;
        let ack = self.stream.read(1)?;
        if ack.first() != Some(&ACK) {
            return Err(Error::Unexpected {
                what: "ident",
                expected: "0x06".to_owned(),
                got: format!("{ack:02x?}"),
            });
        }

        let mut info = RadioInfo::default();
        for (magic, reply_len) in HANDSHAKE {
            self.stream.write_all(magic)?;
            let reply = self.stream.read_exact(reply_len, "handshake reply")?;
            if info.ident.is_empty() {
                info.ident = reply;
            }
        }
        Ok(info)
    }

    fn read_block(&mut self, addr: u16, size: u8) -> Result<Vec<u8>> {
        let command = [b'R', (addr >> 8) as u8, (addr & 0xff) as u8, size];
        self.stream.write_all(&command)?;

        let reply = self
            .stream
            .read_exact(usize::from(size) + 4, "memory block")?;

        // the radio echoes the address and length back, and answering with a
        // different one means the conversation has slipped out of step
        let echoed =
            u16::from(*reply.get(1).unwrap_or(&0)) << 8 | u16::from(*reply.get(2).unwrap_or(&0));
        if echoed != addr || reply.get(3) != Some(&size) {
            return Err(Error::Unexpected {
                what: "memory block header",
                expected: format!("{addr:#06x} {size:#04x}"),
                got: format!("{echoed:#06x} {:#04x?}", reply.get(3)),
            });
        }

        Ok(crypt(self.model.key, reply.get(4..).unwrap_or_default()))
    }
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
    use std::collections::HashMap;

    /// A UV-17Pro that answers the clone protocol from a memory map.
    ///
    /// Strict on purpose: reads outside the regions this model has, or a
    /// command it does not know, are recorded rather than tolerated.
    struct FakeRadio {
        model: &'static Model,
        memory: HashMap<u16, u8>,
        incoming: Vec<u8>,
        outgoing: Vec<u8>,
        identified: bool,
        complaints: Vec<String>,
        blocks: usize,
    }

    impl FakeRadio {
        fn new(model: &'static Model) -> Self {
            let mut memory = HashMap::new();
            let mut value = 0u8;
            for region in model.regions {
                for offset in 0..region.size {
                    memory.insert(region.start.wrapping_add(offset), value);
                    value = value.wrapping_add(1);
                }
            }
            Self {
                model,
                memory,
                incoming: Vec::new(),
                outgoing: Vec::new(),
                identified: false,
                complaints: Vec::new(),
                blocks: 0,
            }
        }

        fn in_region(&self, addr: u16, size: u8) -> bool {
            self.model.regions.iter().any(|r| {
                let start = u32::from(r.start);
                let end = start + u32::from(r.size);
                u32::from(addr) >= start && u32::from(addr) + u32::from(size) <= end
            })
        }

        fn process(&mut self) {
            loop {
                let ident = self.model.ident.as_bytes();

                // a radio answers the ident whenever it is sent, so a second
                // session against the same fake works the way a second run of
                // the tool against a real radio does
                if self.incoming.starts_with(ident) {
                    self.incoming.drain(..ident.len());
                    self.outgoing.push(ACK);
                    self.identified = true;
                    continue;
                }

                if !self.identified {
                    if self.incoming.len() < ident.len() {
                        return;
                    }
                    self.complaints.push("wrong ident magic".to_owned());
                    self.incoming.clear();
                    return;
                }

                let Some(first) = self.incoming.first().copied() else {
                    return;
                };

                match first {
                    0x46 => {
                        self.incoming.remove(0);
                        self.outgoing.extend_from_slice(b"UV17PROFAKE-0001");
                    }
                    0x4d => {
                        self.incoming.remove(0);
                        self.outgoing.extend_from_slice(&[0x30; 15]);
                    }
                    0x53 => {
                        if self.incoming.len() < 25 {
                            return;
                        }
                        self.incoming.drain(..25);
                        self.outgoing.push(ACK);
                    }
                    b'R' => {
                        if self.incoming.len() < 4 {
                            return;
                        }
                        let cmd: Vec<u8> = self.incoming.drain(..4).collect();
                        let addr = u16::from(cmd[1]) << 8 | u16::from(cmd[2]);
                        let size = cmd[3];

                        if !self.in_region(addr, size) {
                            self.complaints
                                .push(format!("read outside a region at {addr:#06x}"));
                            return;
                        }

                        let plain: Vec<u8> = (0..u16::from(size))
                            .map(|o| {
                                self.memory
                                    .get(&addr.wrapping_add(o))
                                    .copied()
                                    .unwrap_or(0xff)
                            })
                            .collect();

                        self.outgoing
                            .extend_from_slice(&[b'R', cmd[1], cmd[2], size]);
                        let encrypted = crypt(self.model.key, &plain);
                        self.outgoing.extend_from_slice(&encrypted);
                        self.blocks += 1;
                    }
                    b'W' => {
                        if self.incoming.len() < 4 {
                            return;
                        }
                        let size = usize::from(self.incoming[3]);
                        if self.incoming.len() < 4 + size {
                            return;
                        }
                        let frame: Vec<u8> = self.incoming.drain(..4 + size).collect();
                        let addr = u16::from(frame[1]) << 8 | u16::from(frame[2]);

                        if !self.in_region(addr, frame[3]) {
                            self.complaints
                                .push(format!("write outside a region at {addr:#06x}"));
                            return;
                        }

                        let plain = crypt(self.model.key, &frame[4..]);
                        for (offset, byte) in plain.iter().enumerate() {
                            self.memory.insert(addr.wrapping_add(offset as u16), *byte);
                        }

                        self.outgoing.push(ACK);
                        self.blocks += 1;
                    }
                    other => {
                        self.complaints
                            .push(format!("unknown command {other:#04x}"));
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
        fn sleep(&mut self, _: u64) {}
    }

    #[test]
    fn the_obfuscation_is_its_own_inverse() {
        let plain: Vec<u8> = (0..=255).collect();
        let once = crypt(1, &plain);
        assert_ne!(once, plain, "nothing was obfuscated");
        assert_eq!(crypt(1, &once), plain, "it did not round trip");
    }

    #[test]
    fn padding_and_key_matching_bytes_are_left_alone() {
        // this is what makes the conditional xor reversible
        assert_eq!(crypt(1, &[0x00, 0xff]), vec![0x00, 0xff]);

        // "CO 7": the third byte of the key is a space, so anything at that
        // position passes through
        let data = [0x41u8, 0x41, 0x41, 0x41];
        let out = crypt(1, &data);
        assert_eq!(out[2], 0x41, "a space in the key means no change");
        assert_ne!(out[0], 0x41, "a real key byte does change it");
    }

    #[test]
    fn an_unknown_key_does_not_panic() {
        assert_eq!(crypt(200, &[1, 2, 3]), vec![1, 2, 3]);
    }

    #[test]
    fn a_download_reads_every_region_and_nothing_else() {
        for m in ALL {
            let radio = FakeRadio::new(m);
            let mut session = Session::new(radio, m);
            let (image, info) = session.download().expect("the download succeeds");

            assert_eq!(
                image.len(),
                m.memory_size() + m.chirp_model.len(),
                "{}: wrong image size",
                m.name
            );
            assert!(
                image.ends_with(m.chirp_model.as_bytes()),
                "{}: the CHIRP model name is missing",
                m.name
            );
            assert_eq!(info.describe(), "UV17PROFAKE-0001", "{}", m.name);
            assert!(
                session.stream.complaints.is_empty(),
                "{}: the radio complained: {:?}",
                m.name,
                session.stream.complaints
            );
        }
    }

    #[test]
    fn the_memory_comes_back_decrypted() {
        let m = model("UV17PRO").expect("UV17PRO is known");
        let radio = FakeRadio::new(m);
        let mut session = Session::new(radio, m);
        let (image, _) = session.download().expect("the download succeeds");

        // the fake fills memory with a rising counter, region by region
        let expected: Vec<u8> = (0..m.memory_size()).map(|ix| ix as u8).collect();
        assert_eq!(
            image.get(..m.memory_size()),
            Some(expected.as_slice()),
            "the image is not the memory the radio holds"
        );
    }

    #[test]
    fn a_short_final_block_is_still_inside_the_region() {
        // 0x01c0 is not a whole number of 0x40 blocks, so the last read of
        // that region has to be short rather than overrun it
        let m = model("UV5RMINI").expect("UV5RMINI is known");
        let radio = FakeRadio::new(m);
        let mut session = Session::new(radio, m);
        session.download().expect("the download succeeds");

        assert!(
            session.stream.complaints.is_empty(),
            "the radio complained: {:?}",
            session.stream.complaints
        );
    }

    #[test]
    fn a_session_can_only_be_used_once() {
        let m = model("UV17PRO").expect("UV17PRO is known");
        let mut session = Session::new(FakeRadio::new(m), m);
        session.download().expect("the first download succeeds");
        assert!(matches!(session.download(), Err(Error::SessionSpent)));
    }

    #[test]
    fn the_wrong_model_is_refused_rather_than_half_read() {
        // UV17PRO answers a different magic from UV5RMINI
        let radio = FakeRadio::new(model("UV17PRO").expect("known"));
        let mut session = Session::new(radio, model("UV5RMINI").expect("known"));
        assert!(session.download().is_err());
    }

    #[test]
    fn every_model_is_consistent() {
        for m in ALL {
            assert_eq!(m.ident.len(), 16, "{}: odd ident length", m.name);
            assert!(!m.regions.is_empty(), "{}: no regions", m.name);
            assert!(m.memory_size() > 0, "{}: no memory", m.name);
            assert!(!m.chirp_model.is_empty(), "{}: no CHIRP name", m.name);
        }
        assert!(model("UV17PRO").is_some());
        assert!(model("NOPE").is_none());
    }

    // -----------------------------------------------------------------
    // Writing a codeplug
    // -----------------------------------------------------------------

    /// The property that matters: what is written is what comes back. If the
    /// obfuscation were applied the wrong way round on either side, a round
    /// trip through one tool would still look right, so the fake enciphers
    /// and deciphers independently of the driver.
    #[test]
    fn what_is_written_reads_back_identically() {
        let model = model("UV5RMINI").expect("a known radio");
        let total = model.memory_size();

        let image: Vec<u8> = (0..total).map(|i| (i % 251) as u8).collect();

        let mut radio = FakeRadio::new(model);
        Session::new(&mut radio, model)
            .upload(&image, |_, _| {})
            .expect("uploads");
        assert!(radio.complaints.is_empty(), "{:?}", radio.complaints);

        let (read_back, _) = Session::new(&mut radio, model)
            .download()
            .expect("downloads");

        // a download appends the model stamp, the memory itself is what is
        // being compared
        assert_eq!(
            &read_back[..total],
            &image[..],
            "the codeplug changed in transit"
        );
    }

    #[test]
    fn a_codeplug_that_is_too_small_is_refused_before_anything_is_sent() {
        let model = model("UV5RMINI").expect("a known radio");
        let mut radio = FakeRadio::new(model);

        let err = Session::new(&mut radio, model)
            .upload(&[0u8; 16], |_, _| {})
            .expect_err("must not write a truncated codeplug");
        assert!(err.to_string().contains("at least"), "{err}");
        assert_eq!(
            radio.blocks, 0,
            "nothing may be sent before the size is checked"
        );
    }

    /// A file with CHIRP's metadata or a model stamp after the memory is
    /// still a valid codeplug, and the extra must not be sent to the radio
    #[test]
    fn anything_after_the_memory_is_not_sent() {
        let model = model("UV5RMINI").expect("a known radio");
        let total = model.memory_size();

        let mut image = vec![0xaau8; total];
        image.extend_from_slice(model.chirp_model.as_bytes());
        image.extend_from_slice(b"\x00\xffchirp\xeeimg\x00\x01 and some metadata");

        let mut radio = FakeRadio::new(model);
        Session::new(&mut radio, model)
            .upload(&image, |_, _| {})
            .expect("uploads");

        assert!(radio.complaints.is_empty(), "{:?}", radio.complaints);
        let written = radio.memory.len();
        assert_eq!(written, total, "only the memory itself is written");
    }

    #[test]
    fn a_radio_that_rejects_a_block_stops_the_upload() {
        struct Grumpy {
            writes: usize,
        }
        impl ByteStream for Grumpy {
            fn write_all(&mut self, data: &[u8]) -> Result<()> {
                if data.first() == Some(&b'W') {
                    self.writes += 1;
                }
                Ok(())
            }
            fn read(&mut self, len: usize) -> Result<Vec<u8>> {
                // acknowledge the handshake, then refuse the third block
                if self.writes >= 3 {
                    return Ok(vec![0x15; len]);
                }
                Ok(vec![ACK; len])
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _millis: u64) {}
        }

        let model = model("UV5RMINI").expect("a known radio");
        let image = vec![0u8; model.memory_size()];

        let err = Session::new(Grumpy { writes: 0 }, model)
            .upload(&image, |_, _| {})
            .expect_err("a refused block is not success");
        assert!(err.to_string().contains("acknowledgement"), "{err}");
    }

    #[test]
    fn upload_progress_adds_up_to_the_memory_size() {
        let model = model("UV5RMINI").expect("a known radio");
        let total = model.memory_size();
        let mut seen = Vec::new();

        let mut radio = FakeRadio::new(model);
        Session::new(&mut radio, model)
            .upload(&vec![0u8; total], |done, all| seen.push((done, all)))
            .expect("uploads");

        assert_eq!(seen.first(), Some(&(0, total)));
        assert_eq!(seen.last(), Some(&(total, total)));
        assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0));
    }

    /// Every real model has regions that divide evenly by the block size, so
    /// a driver that always sends a full block looks correct against all of
    /// them. It would still write past the end of a region on a model that
    /// did not divide evenly, which is memory the region does not cover.
    #[test]
    fn a_region_that_does_not_divide_evenly_is_not_overrun() {
        static ODD_REGIONS: &[Region] = &[
            Region {
                start: 0x0000,
                size: 0x50,
            },
            Region {
                start: 0x1000,
                size: 0x0a,
            },
        ];
        static ODD: Model = Model {
            name: "ODDSIZE",
            ident: "PROGRAMCOLORPROU",
            chirp_model: "Odd",
            regions: ODD_REGIONS,
            key: 0,
        };

        let total = ODD.memory_size();
        assert_eq!(total, 0x5a, "the regions do not divide by the block size");

        let mut radio = FakeRadio::new(&ODD);
        Session::new(&mut radio, &ODD)
            .upload(&vec![0x5au8; total], |_, _| {})
            .expect("uploads");

        assert!(
            radio.complaints.is_empty(),
            "the driver ran past a region: {:?}",
            radio.complaints
        );
        assert_eq!(
            radio.memory.len(),
            total,
            "exactly the regions were written, no more"
        );
    }

    #[test]
    fn a_session_cannot_upload_twice() {
        let model = model("UV5RMINI").expect("a known radio");
        let mut radio = FakeRadio::new(model);
        let mut session = Session::new(&mut radio, model);

        session
            .upload(&vec![0u8; model.memory_size()], |_, _| {})
            .expect("uploads");
        assert!(
            session
                .upload(&vec![0u8; model.memory_size()], |_, _| {})
                .is_err(),
            "a spent session must not be reused"
        );
    }
}
