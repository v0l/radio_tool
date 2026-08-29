//! The H8SX boot protocol, which is how a Yaesu FT-70D takes firmware.
//!
//! The radio speaks this over USB bulk endpoints rather than a serial port,
//! but it is a byte stream either way, so it is written against
//! [`ByteStream`] like everything else here and can be exercised against a
//! fake device.
//!
//! # How much of this is trustworthy
//!
//! Less than the rest of the crate, and it is worth being plain about why.
//! The C++ this replaces came from `h8300-flasher`, which is by the same
//! author, so the two are one lineage rather than two implementations that
//! agree. Both copies are in `references/` for comparison.
//!
//! The consequence is concentrated in the final sum check. In the original,
//! its four conditions were joined with `&&`, so the check could never fail,
//! which means the value it compares against has never been tested against a
//! radio. That expected value is accumulated as
//!
//! ```text
//! bin_sum += checksum(chunk_data, 1024)      // a negated byte sum
//! ```
//!
//! a sum of per chunk two's complement negations, which is not the byte sum
//! a Renesas User MAT sum check is documented to return. Rather than guess,
//! [`flash`] computes both candidates and hands back what the device
//! reported alongside them, so the first person with an FT-70D learns which
//! is right from one run instead of from a failed flash.
//!
//! # Where this departs from the C++
//!
//! The device inquiry response is read using its own length field, and its
//! checksum is verified. The C++ reads a fixed size header and then trusts a
//! count from the device to say how much more to read, with the checksum
//! left as a TODO.

use crate::{ByteStream, Error, Result};

/// Begin the inquiry phase
const BEGIN_INQUIRY: u8 = 0x55;
/// Answer to [`BEGIN_INQUIRY`]
const INQUIRY_OK: u8 = 0xe6;
/// Ask what device this is
const DEVICE_INQUIRY: u8 = 0x20;
/// Choose the device to program
const DEVICE_SELECT: u8 = 0x10;
/// Ask about clock modes
const CLOCK_MODE_INQUIRY: u8 = 0x21;
/// Ask about programming units
const PROG_UNIT_INQUIRY: u8 = 0x27;
/// Confirm the new bit rate
const BITRATE_CONFIRM: u8 = 0x06;
/// Move to the programming and erasing state
const BEGIN_PROGRAMMING: u8 = 0x40;
/// Program the user MAT rather than anything else
const USER_MAT_SELECT: u8 = 0x43;
/// Send a block of data
const PROGRAM: u8 = 0x50;
/// Ask for the sum over what was written
const USER_MAT_CHECKSUM: u8 = 0x4b;
/// Answer to [`USER_MAT_CHECKSUM`]
const USER_MAT_CHECKSUM_REPLY: u8 = 0x5b;
/// Generic acknowledgement
const ACK: u8 = 0x06;

/// Bytes per programming block. The command is named for 128 bytes and
/// carries 1024, which is what the vendor tool does.
const BLOCK: usize = 1024;

/// Fixed frame selecting clock mode 1, with its checksum already in place
const CLOCK_MODE_SELECT: [u8; 4] = [0x11, 0x01, 0x01, 0xed];

/// Fixed frame selecting the bit rate, with its checksum already in place
const BITRATE_SELECT: [u8; 10] = [0x3f, 0x07, 0x04, 0x80, 0x06, 0x40, 0x02, 0x01, 0x01, 0xec];

/// Fixed frame ending the programming run, with its checksum already in place
const PROGRAM_END: [u8; 6] = [PROGRAM, 0xff, 0xff, 0xff, 0xff, 0xb4];

/// What the device says it is
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceId {
    /// Four character device code, which is echoed back to select it
    pub code: [u8; 4],
    /// Human readable part of the device string
    pub name: String,
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", String::from_utf8_lossy(&self.code), self.name)
    }
}

/// The result of the sum check the device performs after programming.
///
/// Both candidate expectations are reported because it is not settled which
/// one the device means. See the module documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SumCheck {
    /// What the device said
    pub reported: u32,
    /// Sum of the per block negated checksums, which is what the vendor
    /// lineage compares against
    pub block_sum: u32,
    /// Plain sum of every byte written, padding included, which is what a
    /// Renesas user MAT sum check is documented to return
    pub byte_sum: u32,
}

impl SumCheck {
    /// Does the device agree with either candidate
    pub fn agrees(&self) -> bool {
        self.reported == self.block_sum || self.reported == self.byte_sum
    }

    /// Which candidate the device agreed with, for reporting
    pub fn explain(&self) -> String {
        if self.reported == self.byte_sum && self.reported == self.block_sum {
            "both candidate sums agree, which says nothing about which is meant".to_owned()
        } else if self.reported == self.byte_sum {
            "the device means a plain byte sum".to_owned()
        } else if self.reported == self.block_sum {
            "the device means the sum of block checksums".to_owned()
        } else {
            format!(
                "the device reported {:#010x}, which matches neither a byte sum \
                 ({:#010x}) nor a sum of block checksums ({:#010x})",
                self.reported, self.byte_sum, self.block_sum
            )
        }
    }
}

/// Progress through a flash
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes accepted so far
    pub sent: usize,
    /// Bytes in total
    pub total: usize,
}

/// The checksum this protocol uses everywhere: the two's complement negation
/// of the sum of the bytes, so that a frame and its checksum sum to zero
fn checksum(data: &[u8]) -> u8 {
    data.iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        .wrapping_neg()
}

/// Ask the device what it is
pub fn identify<S: ByteStream>(stream: &mut S) -> Result<DeviceId> {
    stream.flush_input()?;

    stream.write_all(&[BEGIN_INQUIRY])?;
    let reply = stream.read_exact(1, "answer to the inquiry")?;
    match reply.first() {
        Some(&INQUIRY_OK) => {}
        Some(other) => {
            return Err(Error::Unexpected {
                what: "answer to the inquiry",
                expected: format!("{INQUIRY_OK:#04x}"),
                got: format!("{other:#04x}"),
            });
        }
        None => return Err(Error::NoRadio),
    }

    stream.write_all(&[DEVICE_INQUIRY])?;
    let body = read_framed(stream, "device inquiry")?;

    // ndev, nchar, then nchar characters of device string
    let nchar = usize::from(*body.get(1).ok_or(Error::Unexpected {
        what: "device inquiry",
        expected: "a device count and a name length".to_owned(),
        got: "a response too short to hold them".to_owned(),
    })?);

    let text = body.get(2..2 + nchar).ok_or(Error::Unexpected {
        what: "device inquiry",
        expected: format!("{nchar} characters of device name"),
        got: format!("{} bytes of response", body.len().saturating_sub(2)),
    })?;

    let code_bytes = text.get(..4).ok_or(Error::Unexpected {
        what: "device inquiry",
        expected: "a four character device code".to_owned(),
        got: format!("{} characters", text.len()),
    })?;

    let mut code = [0u8; 4];
    code.copy_from_slice(code_bytes);

    Ok(DeviceId {
        code,
        name: String::from_utf8_lossy(text.get(4..).unwrap_or_default())
            .trim_end_matches('\0')
            .to_owned(),
    })
}

/// Write firmware to the device.
///
/// The image is sent in 1024 byte blocks from address zero, with a short
/// final block padded with `0xff`.
pub fn flash<S: ByteStream>(
    stream: &mut S,
    data: &[u8],
    mut progress: impl FnMut(Progress),
) -> Result<SumCheck> {
    let device = identify(stream)?;

    // select the device, echoing the code it gave us
    let mut select = vec![DEVICE_SELECT, 4];
    select.extend_from_slice(&device.code);
    select.push(checksum(&select));
    stream.write_all(&select)?;
    expect_ack(stream, "device selection")?;

    // these two inquiries are made and their answers discarded, as the
    // vendor tool does, but the frames still have to be consumed
    stream.write_all(&[CLOCK_MODE_INQUIRY])?;
    read_framed(stream, "clock mode inquiry")?;

    stream.write_all(&CLOCK_MODE_SELECT)?;
    expect_ack(stream, "clock mode selection")?;

    stream.write_all(&[PROG_UNIT_INQUIRY])?;
    read_framed(stream, "programming unit inquiry")?;

    stream.write_all(&BITRATE_SELECT)?;
    expect_ack(stream, "bit rate selection")?;

    stream.write_all(&[BITRATE_CONFIRM])?;
    expect_ack(stream, "bit rate confirmation")?;

    stream.write_all(&[BEGIN_PROGRAMMING])?;
    expect_ack(stream, "transition to programming state")?;

    stream.write_all(&[USER_MAT_SELECT])?;
    expect_ack(stream, "user MAT selection")?;

    let mut block_sum: u32 = 0;
    let mut byte_sum: u32 = 0;
    let mut sent = 0usize;

    progress(Progress {
        sent: 0,
        total: data.len(),
    });

    for (index, chunk) in data.chunks(BLOCK).enumerate() {
        let address = (index * BLOCK) as u32;

        let mut payload = [0xffu8; BLOCK];
        payload
            .get_mut(..chunk.len())
            .ok_or(Error::Port("block longer than a block".to_owned()))?
            .copy_from_slice(chunk);

        let mut frame = Vec::with_capacity(BLOCK + 6);
        frame.push(PROGRAM);
        frame.extend_from_slice(&address.to_be_bytes());
        frame.extend_from_slice(&payload);
        frame.push(checksum(&frame));

        stream.write_all(&frame)?;
        expect_ack(stream, "programming a block")?;

        block_sum = block_sum.wrapping_add(u32::from(checksum(&payload)));
        byte_sum = payload
            .iter()
            .fold(byte_sum, |acc, b| acc.wrapping_add(u32::from(*b)));

        sent += chunk.len();
        progress(Progress {
            sent,
            total: data.len(),
        });
    }

    stream.write_all(&PROGRAM_END)?;
    expect_ack(stream, "end of programming")?;

    stream.write_all(&[USER_MAT_CHECKSUM])?;
    let reply = stream.read_exact(7, "sum check")?;

    let head = reply.first().copied().unwrap_or(0);
    if head != USER_MAT_CHECKSUM_REPLY {
        return Err(Error::Unexpected {
            what: "sum check",
            expected: format!("{USER_MAT_CHECKSUM_REPLY:#04x}"),
            got: format!("{head:#04x}"),
        });
    }
    if reply.get(1) != Some(&4) {
        return Err(Error::Unexpected {
            what: "sum check",
            expected: "a four byte sum".to_owned(),
            got: format!("a length of {:?}", reply.get(1)),
        });
    }
    if checksum(reply.get(..6).unwrap_or_default()) != reply.get(6).copied().unwrap_or(0) {
        return Err(Error::Unexpected {
            what: "sum check",
            expected: "a frame whose checksum is right".to_owned(),
            got: "a corrupt frame".to_owned(),
        });
    }

    let reported = u32::from_be_bytes([
        reply.get(2).copied().unwrap_or(0),
        reply.get(3).copied().unwrap_or(0),
        reply.get(4).copied().unwrap_or(0),
        reply.get(5).copied().unwrap_or(0),
    ]);

    Ok(SumCheck {
        reported,
        block_sum,
        byte_sum,
    })
}

/// Read a reply of the form command, length, body, checksum, and check it
fn read_framed<S: ByteStream>(stream: &mut S, what: &'static str) -> Result<Vec<u8>> {
    let head = stream.read_exact(2, what)?;
    let length = usize::from(head.get(1).copied().unwrap_or(0));

    let body = stream.read_exact(length, what)?;
    let sum = stream.read_exact(1, what)?;

    let mut whole = head.clone();
    whole.extend_from_slice(&body);

    if checksum(&whole) != sum.first().copied().unwrap_or(0) {
        return Err(Error::Unexpected {
            what,
            expected: "a frame whose checksum is right".to_owned(),
            got: "a corrupt frame".to_owned(),
        });
    }

    Ok(body)
}

fn expect_ack<S: ByteStream>(stream: &mut S, what: &'static str) -> Result<()> {
    let reply = stream.read_exact(1, what)?;
    match reply.first() {
        Some(&ACK) => Ok(()),
        Some(other) => Err(Error::Unexpected {
            what,
            expected: format!("{ACK:#04x}"),
            got: format!("{other:#04x}"),
        }),
        None => Err(Error::NoRadio),
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

    /// A device that follows the protocol and reassembles what it is sent
    struct Device {
        inbox: Vec<u8>,
        outbox: Vec<u8>,
        pub written: Vec<u8>,
        pub addresses: Vec<u32>,
        pub finished: bool,
        /// What the device claims its sum is, chosen by the test
        pub report: fn(&Device) -> u32,
        /// Refuse the block at this index
        pub refuse_block: Option<usize>,
        /// Corrupt the checksum of the sum check reply
        pub corrupt_sum_frame: bool,
        blocks: usize,
        state: State,
    }

    #[derive(PartialEq, Eq, Debug)]
    enum State {
        Idle,
        Inquired,
        Programming,
    }

    fn framed(command: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![command, body.len() as u8];
        out.extend_from_slice(body);
        out.push(checksum(&out));
        out
    }

    impl Device {
        fn new() -> Self {
            Self {
                inbox: Vec::new(),
                outbox: Vec::new(),
                written: Vec::new(),
                addresses: Vec::new(),
                finished: false,
                report: |d| d.byte_sum(),
                refuse_block: None,
                corrupt_sum_frame: false,
                blocks: 0,
                state: State::Idle,
            }
        }

        fn byte_sum(&self) -> u32 {
            self.written
                .iter()
                .fold(0u32, |acc, b| acc.wrapping_add(u32::from(*b)))
        }

        fn block_sum(&self) -> u32 {
            self.written
                .chunks(BLOCK)
                .fold(0u32, |acc, c| acc.wrapping_add(u32::from(checksum(c))))
        }

        fn digest(&mut self) {
            loop {
                let Some(&command) = self.inbox.first() else {
                    return;
                };

                match command {
                    BEGIN_INQUIRY if self.state == State::Idle => {
                        self.inbox.remove(0);
                        self.outbox.push(INQUIRY_OK);
                        self.state = State::Inquired;
                    }
                    DEVICE_INQUIRY => {
                        self.inbox.remove(0);
                        // one device, an eight character string
                        let mut body = vec![1u8, 8];
                        body.extend_from_slice(b"3745FT70");
                        self.outbox.extend_from_slice(&framed(0x30, &body));
                    }
                    DEVICE_SELECT => {
                        if self.inbox.len() < 7 {
                            return;
                        }
                        let frame: Vec<u8> = self.inbox.drain(..7).collect();
                        if checksum(&frame[..6]) == frame[6] {
                            self.outbox.push(ACK);
                        }
                    }
                    CLOCK_MODE_INQUIRY | PROG_UNIT_INQUIRY => {
                        self.inbox.remove(0);
                        self.outbox.extend_from_slice(&framed(command + 0x10, &[1]));
                    }
                    0x11 => {
                        if self.inbox.len() < 4 {
                            return;
                        }
                        self.inbox.drain(..4);
                        self.outbox.push(ACK);
                    }
                    0x3f => {
                        if self.inbox.len() < 10 {
                            return;
                        }
                        self.inbox.drain(..10);
                        self.outbox.push(ACK);
                    }
                    BITRATE_CONFIRM | BEGIN_PROGRAMMING | USER_MAT_SELECT => {
                        self.inbox.remove(0);
                        if command == USER_MAT_SELECT {
                            self.state = State::Programming;
                        }
                        self.outbox.push(ACK);
                    }
                    PROGRAM => {
                        // either a data block or the frame ending the run
                        if self.inbox.len() >= 6 && self.inbox[1..5] == [0xff; 4] {
                            self.inbox.drain(..6);
                            self.finished = true;
                            self.outbox.push(ACK);
                            continue;
                        }
                        if self.inbox.len() < BLOCK + 6 {
                            return;
                        }
                        let frame: Vec<u8> = self.inbox.drain(..BLOCK + 6).collect();
                        let sum_ok = checksum(&frame[..BLOCK + 5]) == frame[BLOCK + 5];

                        if !sum_ok || self.refuse_block == Some(self.blocks) {
                            self.outbox.push(0x15);
                            self.blocks += 1;
                            continue;
                        }

                        self.addresses
                            .push(u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]));
                        self.written.extend_from_slice(&frame[5..BLOCK + 5]);
                        self.blocks += 1;
                        self.outbox.push(ACK);
                    }
                    USER_MAT_CHECKSUM => {
                        self.inbox.remove(0);
                        let value = (self.report)(self);
                        let mut frame = vec![USER_MAT_CHECKSUM_REPLY, 4];
                        frame.extend_from_slice(&value.to_be_bytes());
                        let mut sum = checksum(&frame);
                        if self.corrupt_sum_frame {
                            sum ^= 0xff;
                        }
                        frame.push(sum);
                        self.outbox.extend_from_slice(&frame);
                    }
                    _ => {
                        self.inbox.remove(0);
                    }
                }
            }
        }
    }

    impl ByteStream for Device {
        fn write_all(&mut self, data: &[u8]) -> Result<()> {
            self.inbox.extend_from_slice(data);
            self.digest();
            Ok(())
        }
        fn read(&mut self, len: usize) -> Result<Vec<u8>> {
            let take = len.min(self.outbox.len());
            Ok(self.outbox.drain(..take).collect())
        }
        fn flush_input(&mut self) -> Result<()> {
            Ok(())
        }
        fn sleep(&mut self, _millis: u64) {}
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    /// The vendor's fixed frames carry checksums computed by the vendor, so
    /// they check this implementation against the vendor rather than against
    /// itself
    #[test]
    fn the_checksum_matches_the_vendors_own_frames() {
        assert_eq!(
            checksum(&CLOCK_MODE_SELECT[..3]),
            CLOCK_MODE_SELECT[3],
            "clock mode selection"
        );
        assert_eq!(
            checksum(&BITRATE_SELECT[..9]),
            BITRATE_SELECT[9],
            "bit rate selection"
        );
        assert_eq!(
            checksum(&PROGRAM_END[..5]),
            PROGRAM_END[5],
            "end of programming"
        );
    }

    #[test]
    fn a_frame_and_its_checksum_sum_to_zero() {
        for frame in [
            &CLOCK_MODE_SELECT[..],
            &BITRATE_SELECT[..],
            &PROGRAM_END[..],
        ] {
            let total = frame.iter().fold(0u8, |a, b| a.wrapping_add(*b));
            assert_eq!(total, 0, "that is what this checksum is for");
        }
    }

    #[test]
    fn the_device_identifies_itself() {
        let mut device = Device::new();
        let id = identify(&mut device).expect("identifies");
        assert_eq!(&id.code, b"3745");
        assert_eq!(id.name, "FT70");
        assert_eq!(id.to_string(), "3745-FT70");
    }

    #[test]
    fn firmware_arrives_byte_for_byte() {
        for len in [1, 1023, 1024, 1025, 4096, 10_000] {
            let data = body(len);
            let mut device = Device::new();

            let sum = flash(&mut device, &data, |_| {})
                .unwrap_or_else(|e| panic!("{len} bytes failed: {e}"));

            let blocks = len.div_ceil(BLOCK);
            assert_eq!(device.written.len(), blocks * BLOCK, "{len} bytes");
            assert_eq!(&device.written[..len], &data[..], "{len} bytes");
            assert!(
                device.written[len..].iter().all(|b| *b == 0xff),
                "the tail is padded with 0xff"
            );
            assert!(device.finished, "the run was ended");
            assert!(sum.agrees(), "{len} bytes: {}", sum.explain());
        }
    }

    #[test]
    fn blocks_are_addressed_from_zero_in_order() {
        let mut device = Device::new();
        flash(&mut device, &body(BLOCK * 3 + 7), |_| {}).expect("flashes");
        assert_eq!(
            device.addresses,
            vec![0, 1024, 2048, 3072],
            "addresses are big endian byte offsets"
        );
    }

    /// The point of reporting both candidates: whichever the device means,
    /// the answer is legible rather than a bare failure
    #[test]
    fn both_candidate_sums_are_reported() {
        let data = body(3000);

        let mut device = Device::new();
        device.report = |d| d.byte_sum();
        let sum = flash(&mut device, &data, |_| {}).expect("flashes");
        assert!(sum.agrees());
        assert_eq!(sum.explain(), "the device means a plain byte sum");

        let mut device = Device::new();
        device.report = |d| d.block_sum();
        let sum = flash(&mut device, &data, |_| {}).expect("flashes");
        assert!(sum.agrees());
        assert_eq!(sum.explain(), "the device means the sum of block checksums");

        let mut device = Device::new();
        device.report = |_| 0xdead_beef;
        let sum = flash(&mut device, &data, |_| {}).expect("flashes");
        assert!(!sum.agrees(), "a flash that matches neither is not a pass");
        assert!(
            sum.explain().contains("matches neither"),
            "{}",
            sum.explain()
        );
    }

    /// The two candidates have to actually differ, or the test above proves
    /// nothing
    #[test]
    fn the_two_candidate_sums_are_not_the_same_number() {
        let mut device = Device::new();
        flash(&mut device, &body(5000), |_| {}).expect("flashes");
        assert_ne!(device.byte_sum(), device.block_sum());
    }

    #[test]
    fn a_corrupt_sum_check_reply_is_refused() {
        let mut device = Device::new();
        device.corrupt_sum_frame = true;

        let err = flash(&mut device, &body(2048), |_| {}).expect_err("must not be trusted");
        assert!(err.to_string().contains("corrupt"), "{err}");
    }

    #[test]
    fn a_refused_block_stops_the_flash() {
        let mut device = Device::new();
        device.refuse_block = Some(2);

        let err = flash(&mut device, &body(BLOCK * 5), |_| {}).expect_err("must not claim success");
        assert!(err.to_string().contains("programming a block"), "{err}");
    }

    #[test]
    fn a_device_that_says_nothing_is_not_mistaken_for_success() {
        struct Silent;
        impl ByteStream for Silent {
            fn write_all(&mut self, _data: &[u8]) -> Result<()> {
                Ok(())
            }
            fn read(&mut self, _len: usize) -> Result<Vec<u8>> {
                Ok(Vec::new())
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _millis: u64) {}
        }

        assert!(identify(&mut Silent).is_err());
        assert!(flash(&mut Silent, &body(100), |_| {}).is_err());
    }

    #[test]
    fn a_corrupt_reply_is_refused_rather_than_parsed() {
        struct Corrupt {
            outbox: Vec<u8>,
        }
        impl ByteStream for Corrupt {
            fn write_all(&mut self, data: &[u8]) -> Result<()> {
                if data.first() == Some(&BEGIN_INQUIRY) {
                    self.outbox.push(INQUIRY_OK);
                } else if data.first() == Some(&DEVICE_INQUIRY) {
                    let mut frame = framed(
                        0x30,
                        &[1, 8, b'3', b'7', b'4', b'5', b'F', b'T', b'7', b'0'],
                    );
                    let last = frame.len() - 1;
                    frame[last] ^= 0xff; // break the checksum
                    self.outbox.extend_from_slice(&frame);
                }
                Ok(())
            }
            fn read(&mut self, len: usize) -> Result<Vec<u8>> {
                let take = len.min(self.outbox.len());
                Ok(self.outbox.drain(..take).collect())
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _millis: u64) {}
        }

        let err = identify(&mut Corrupt { outbox: Vec::new() }).expect_err("refused");
        assert!(err.to_string().contains("corrupt"), "{err}");
    }

    /// A length that came from the device must not be trusted to index a
    /// buffer, which is the bug the C++ had to be fixed for
    #[test]
    fn a_name_longer_than_the_response_is_refused() {
        struct Liar;
        impl ByteStream for Liar {
            fn write_all(&mut self, data: &[u8]) -> Result<()> {
                let _ = data;
                Ok(())
            }
            fn read(&mut self, len: usize) -> Result<Vec<u8>> {
                // claims 200 characters of name in a 4 byte body
                let mut body = vec![1u8, 200];
                body.extend_from_slice(b"37");
                let frame = framed(0x30, &body);
                Ok(frame.into_iter().take(len).collect())
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _millis: u64) {}
        }

        // it never gets as far as the name, but it must fail rather than read
        // out of bounds
        assert!(identify(&mut Liar).is_err());
    }

    #[test]
    fn progress_adds_up_to_the_whole_image() {
        let mut seen = Vec::new();
        let mut device = Device::new();
        flash(&mut device, &body(5000), |p| seen.push(p)).expect("flashes");

        assert_eq!(
            seen.first(),
            Some(&Progress {
                sent: 0,
                total: 5000
            })
        );
        assert_eq!(
            seen.last(),
            Some(&Progress {
                sent: 5000,
                total: 5000
            })
        );
        assert!(seen.windows(2).all(|w| w[0].sent <= w[1].sent));
    }

    #[test]
    fn an_empty_image_writes_nothing_but_still_completes() {
        let mut device = Device::new();
        flash(&mut device, &[], |_| {}).expect("completes");
        assert!(device.written.is_empty());
        assert!(device.finished);
    }
}
