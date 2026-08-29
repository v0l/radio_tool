//! YMODEM, which is how the Ailunce HD1 takes a firmware image.
//!
//! Unlike the clone protocols in this crate, this one is a published
//! standard rather than something recovered from a vendor tool, so it can be
//! held to the spec instead of to whatever one implementation happens to do.
//!
//! A transfer runs:
//!
//! ```text
//! receiver                     sender
//!    -> C
//!                              <- block 0: filename, size
//!    -> ACK, then C
//!                              <- block 1..n of data
//!    -> ACK after each
//!                              <- EOT
//!    -> ACK, then C
//!                              <- block 0, all zeroes, ending the batch
//!    -> ACK
//! ```
//!
//! Two places where this deliberately does not copy the `fymodem` C library
//! the C++ used:
//!
//! * A final data block shorter than 1024 bytes is padded with `0x1a`, which
//!   is what the spec asks for. `fymodem` transmits 1024 bytes regardless and
//!   so reads past the end of the caller's buffer, sending whatever happened
//!   to be in memory. A receiver truncates to the size in block 0 either way,
//!   so this is about not reading out of bounds rather than about the bytes
//!   on the wire.
//! * Every step gives up after a bounded number of tries. `fymodem` retries a
//!   rejected data block forever, so a receiver that keeps saying no hangs the
//!   transfer with no way out.

use crate::{ByteStream, Error, Result};
use std::time::{Duration, Instant};

/// Start of a 128 byte block
const SOH: u8 = 0x01;
/// Start of a 1024 byte block
const STX: u8 = 0x02;
/// End of transmission
const EOT: u8 = 0x04;
/// Block accepted
const ACK: u8 = 0x06;
/// Two in a row abort the transfer
const CAN: u8 = 0x18;
/// The receiver asking for a CRC mode transfer
const CRC: u8 = b'C';
/// Padding for a short final block, per the spec
const PAD: u8 = 0x1a;

/// Bytes in the header block
const SHORT_BLOCK: usize = 128;
/// Bytes in a data block
const LONG_BLOCK: usize = 1024;

/// How a transfer behaves when the receiver is slow or unhappy
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// How long to wait for a reply to a block
    pub reply_timeout: Duration,
    /// How long to wait for the receiver to ask for the transfer
    pub handshake_timeout: Duration,
    /// How many times to resend a block the receiver rejects
    pub retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            reply_timeout: Duration::from_millis(2000),
            handshake_timeout: Duration::from_millis(10000),
            retries: 5,
        }
    }
}

/// Progress through a transfer, for a caller that wants to show it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes accepted by the receiver so far
    pub sent: usize,
    /// Bytes in total
    pub total: usize,
}

/// Send one file, which is a whole YMODEM batch: the file and the empty
/// block that closes it.
///
/// The receiver drives the start, so this waits for it to ask.
pub fn send<S: ByteStream>(
    stream: &mut S,
    filename: &str,
    data: &[u8],
    config: &Config,
    mut progress: impl FnMut(Progress),
) -> Result<()> {
    stream.flush_input()?;

    // the receiver opens with C to ask for a CRC transfer
    await_byte(stream, CRC, config.handshake_timeout, "start of transfer")?;

    send_block(
        stream,
        0,
        &header_block(filename, data.len()),
        config,
        "header block",
    )?;
    // having taken the header it asks again, this time for the data
    await_byte(stream, CRC, config.reply_timeout, "request for data")?;

    let mut block = 1u32;
    let mut sent = 0usize;
    progress(Progress {
        sent: 0,
        total: data.len(),
    });

    for chunk in data.chunks(LONG_BLOCK) {
        let mut payload = [PAD; LONG_BLOCK];
        payload
            .get_mut(..chunk.len())
            .ok_or(Error::Port("block longer than a block".to_owned()))?
            .copy_from_slice(chunk);

        send_block(stream, block, &payload, config, "data block")?;

        sent += chunk.len();
        block = block.wrapping_add(1);
        progress(Progress {
            sent,
            total: data.len(),
        });
    }

    // end of file, acknowledged, then the receiver asks for the next file
    let mut tries = 0;
    loop {
        stream.write_all(&[EOT])?;
        match read_byte(stream, config.reply_timeout)? {
            Some(ACK) => break,
            Some(CAN) => return Err(aborted("end of transmission")),
            _ => {
                tries += 1;
                if tries >= config.retries {
                    return Err(Error::Timeout {
                        what: "acknowledgement of end of transmission",
                        wanted: 1,
                        got: 0,
                    });
                }
            }
        }
    }

    await_byte(stream, CRC, config.reply_timeout, "request to close batch")?;

    // an all zero header block says there are no more files
    send_block(stream, 0, &[0u8; SHORT_BLOCK], config, "close of batch")?;

    Ok(())
}

/// Block 0: the name as a C string, then the size in decimal, zero padded
fn header_block(filename: &str, size: usize) -> [u8; SHORT_BLOCK] {
    let mut block = [0u8; SHORT_BLOCK];
    let mut at = 0;

    // leave room for the size and its terminator
    let room = SHORT_BLOCK.saturating_sub(18);
    for byte in filename.bytes().take(room) {
        if let Some(slot) = block.get_mut(at) {
            *slot = byte;
            at += 1;
        }
    }
    at += 1; // the name's terminating zero, already there

    for byte in size.to_string().into_bytes() {
        if let Some(slot) = block.get_mut(at) {
            *slot = byte;
            at += 1;
        }
    }

    block
}

/// Send one block and wait for it to be taken, sending it again while the
/// receiver rejects it.
///
/// The retry limit is the point of this: a receiver that rejects a block
/// forever must end the transfer with an error rather than keep the caller
/// here indefinitely, which is what the C library this replaces does.
fn send_block<S: ByteStream>(
    stream: &mut S,
    number: u32,
    payload: &[u8],
    config: &Config,
    what: &'static str,
) -> Result<()> {
    let start = if payload.len() == SHORT_BLOCK {
        SOH
    } else {
        STX
    };

    // block numbers wrap at 256, which is what the sequence check compares
    let seq = (number & 0xff) as u8;

    let mut frame = Vec::with_capacity(payload.len() + 5);
    frame.push(start);
    frame.push(seq);
    frame.push(!seq);
    frame.extend_from_slice(payload);
    frame.extend_from_slice(&crc16(payload).to_be_bytes());

    for _ in 0..config.retries.max(1) {
        stream.write_all(&frame)?;

        match read_byte(stream, config.reply_timeout)? {
            Some(ACK) => return Ok(()),
            Some(CAN) => return Err(aborted(what)),
            // a rejection, silence, or anything unrecognised all mean the
            // same thing here: send it again
            Some(_) | None => {}
        }
    }

    Err(Error::Unexpected {
        what,
        expected: "an acknowledgement".to_owned(),
        got: format!("rejected {} times", config.retries.max(1)),
    })
}

/// Wait for one particular byte, ignoring anything else that turns up.
///
/// A receiver often sends several C characters before the sender is ready,
/// and leftovers from a previous attempt are common, so anything unrecognised
/// is skipped rather than treated as a failure.
fn await_byte<S: ByteStream>(
    stream: &mut S,
    wanted: u8,
    timeout: Duration,
    what: &'static str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut cancels = 0;

    while Instant::now() < deadline {
        match read_byte(stream, Duration::from_millis(200))? {
            Some(b) if b == wanted => return Ok(()),
            Some(CAN) => {
                cancels += 1;
                if cancels >= 2 {
                    return Err(aborted(what));
                }
            }
            Some(_) => cancels = 0,
            None => {}
        }
    }

    Err(Error::Timeout {
        what,
        wanted: 1,
        got: 0,
    })
}

fn read_byte<S: ByteStream>(stream: &mut S, timeout: Duration) -> Result<Option<u8>> {
    let deadline = Instant::now() + timeout;
    loop {
        let got = stream.read(1)?;
        if let Some(b) = got.first() {
            return Ok(Some(*b));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        stream.sleep(5);
    }
}

fn aborted(what: &'static str) -> Error {
    Error::Unexpected {
        what,
        expected: "an acknowledgement".to_owned(),
        got: "the receiver cancelled the transfer".to_owned(),
    }
}

/// The CRC-16 that XMODEM and YMODEM use: polynomial 0x1021, starting at zero
fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for byte in data {
        let mut x = (crc >> 8) ^ u16::from(*byte);
        x ^= x >> 4;
        crc = (crc << 8) ^ (x << 12) ^ (x << 5) ^ x;
    }
    crc
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

    /// Block rejected. The sender needs no special case for this, since it
    /// treats anything that is not an acknowledgement as a rejection, but a
    /// receiver has to send something.
    const NAK: u8 = 0x15;

    /// A receiver that follows the protocol, so a transfer against it either
    /// reassembles the file exactly or fails
    struct Receiver {
        /// Bytes the sender has written that are not yet parsed
        inbox: Vec<u8>,
        /// Bytes waiting for the sender to read
        outbox: Vec<u8>,
        /// What has been reassembled
        pub file: Vec<u8>,
        pub name: String,
        pub size: usize,
        pub finished: bool,
        /// Reject this many blocks before accepting, to exercise retries
        pub reject_next: usize,
        /// Cancel the transfer at the first opportunity
        pub cancel: bool,
        expecting: u8,
        /// True between the header block and the end of transmission. Without
        /// this, block 256 wraps to sequence zero and looks like a header.
        in_file: bool,
    }

    impl Receiver {
        fn new() -> Self {
            Self {
                inbox: Vec::new(),
                // the receiver speaks first
                outbox: vec![CRC],
                file: Vec::new(),
                name: String::new(),
                size: 0,
                finished: false,
                reject_next: 0,
                cancel: false,
                expecting: 0,
                in_file: false,
            }
        }

        /// Parse whatever complete frames have arrived
        fn digest(&mut self) {
            loop {
                let Some(&start) = self.inbox.first() else {
                    return;
                };

                if start == EOT {
                    self.inbox.remove(0);
                    self.outbox.push(ACK);
                    // the file is over, so the next thing expected is another
                    // header block, which is block zero again
                    self.expecting = 0;
                    self.in_file = false;
                    self.outbox.push(CRC);
                    continue;
                }

                let payload_len = match start {
                    SOH => SHORT_BLOCK,
                    STX => LONG_BLOCK,
                    _ => {
                        // not a frame we know, drop it so a test cannot spin
                        self.inbox.remove(0);
                        continue;
                    }
                };

                let frame_len = payload_len + 5;
                if self.inbox.len() < frame_len {
                    return;
                }

                let frame: Vec<u8> = self.inbox.drain(..frame_len).collect();
                let seq = frame[1];
                let seq_complement = frame[2];
                let payload = &frame[3..3 + payload_len];
                let crc = u16::from_be_bytes([frame[frame_len - 2], frame[frame_len - 1]]);

                if self.cancel {
                    self.outbox.push(CAN);
                    self.outbox.push(CAN);
                    continue;
                }

                if seq != !seq_complement || crc != crc16(payload) || seq != self.expecting {
                    self.outbox.push(NAK);
                    continue;
                }

                if self.reject_next > 0 {
                    self.reject_next -= 1;
                    self.outbox.push(NAK);
                    continue;
                }

                if seq == 0 && !self.in_file && self.name.is_empty() {
                    // the header block: name, then size in decimal
                    let name_end = payload.iter().position(|b| *b == 0).unwrap_or(0);
                    self.name = String::from_utf8_lossy(&payload[..name_end]).into_owned();
                    let rest = &payload[name_end + 1..];
                    let size_end = rest.iter().position(|b| *b == 0).unwrap_or(0);
                    self.size = String::from_utf8_lossy(&rest[..size_end])
                        .parse()
                        .unwrap_or(0);
                    self.expecting = 1;
                    self.in_file = true;
                    self.outbox.push(ACK);
                    // and now ask for the data
                    self.outbox.push(CRC);
                } else if seq == 0 && !self.in_file {
                    // the empty block closing the batch
                    self.finished = true;
                    self.outbox.push(ACK);
                } else {
                    self.file.extend_from_slice(payload);
                    self.expecting = self.expecting.wrapping_add(1);
                    self.outbox.push(ACK);
                }
            }
        }

        /// The file as the receiver would save it, truncated to the size the
        /// header declared
        fn saved(&self) -> Vec<u8> {
            self.file.iter().copied().take(self.size).collect()
        }
    }

    impl ByteStream for Receiver {
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

    fn quick() -> Config {
        Config {
            reply_timeout: Duration::from_millis(50),
            handshake_timeout: Duration::from_millis(50),
            retries: 5,
        }
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn the_crc_matches_the_known_answer() {
        // the check value every XMODEM CRC implementation is tested against
        assert_eq!(crc16(b"123456789"), 0x31C3);
        assert_eq!(crc16(&[]), 0);
        assert_eq!(crc16(&[0u8; 128]), 0);
    }

    #[test]
    fn a_file_arrives_byte_for_byte() {
        for len in [1, 127, 128, 129, 1023, 1024, 1025, 4096, 5000] {
            let data = body(len);
            let mut rx = Receiver::new();
            send(&mut rx, "firmware.bin", &data, &quick(), |_| {})
                .unwrap_or_else(|e| panic!("{len} bytes failed: {e}"));

            assert_eq!(rx.name, "firmware.bin", "{len} bytes");
            assert_eq!(rx.size, len, "{len} bytes");
            assert_eq!(rx.saved(), data, "{len} bytes did not arrive intact");
            assert!(rx.finished, "{len} bytes did not close the batch");
        }
    }

    #[test]
    fn a_short_final_block_is_padded_rather_than_read_past() {
        // 1025 bytes is one full block and one byte, so the last block is
        // almost entirely padding
        let data = body(1025);
        let mut rx = Receiver::new();
        send(&mut rx, "fw", &data, &quick(), |_| {}).expect("sends");

        assert_eq!(rx.file.len(), 2048, "two full blocks go on the wire");
        assert_eq!(rx.saved(), data, "but only the declared size is the file");
        assert!(
            rx.file[1025..].iter().all(|b| *b == 0x1a),
            "the padding is 0x1a as the spec asks, not whatever was next in memory"
        );
    }

    /// Block numbers are a single byte, so anything over 255 blocks, which is
    /// only 261 KB and so any real firmware image, has to wrap round to zero
    #[test]
    fn block_numbers_wrap_at_256() {
        let data = body(LONG_BLOCK * 300);
        let mut rx = Receiver::new();

        send(&mut rx, "big.bin", &data, &quick(), |_| {}).expect("sends 300 blocks");
        assert_eq!(rx.saved(), data, "the file survived the wrap");
        assert!(rx.finished);
    }

    #[test]
    fn a_rejected_block_is_sent_again() {
        let data = body(3000);
        let mut rx = Receiver::new();
        rx.reject_next = 2;

        send(&mut rx, "fw", &data, &quick(), |_| {}).expect("retries and succeeds");
        assert_eq!(rx.saved(), data);
    }

    #[test]
    fn a_cancelling_receiver_stops_the_transfer() {
        let data = body(2048);
        let mut rx = Receiver::new();
        rx.cancel = true;

        let err = send(&mut rx, "fw", &data, &quick(), |_| {}).expect_err("must not claim success");
        assert!(
            err.to_string().contains("cancel"),
            "the error should say the receiver cancelled, got: {err}"
        );
    }

    /// A receiver that never says anything
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

    #[test]
    fn a_silent_receiver_times_out_rather_than_hanging() {
        let started = Instant::now();
        let err = send(&mut Silent, "fw", &body(100), &quick(), |_| {}).expect_err("times out");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "gave up promptly"
        );
        assert!(err.to_string().contains("timed out"), "got: {err}");
    }

    /// A receiver that rejects everything forever, which is what hangs the C
    /// library this replaces
    struct Stubborn {
        outbox: Vec<u8>,
        writes: usize,
    }
    impl ByteStream for Stubborn {
        fn write_all(&mut self, _data: &[u8]) -> Result<()> {
            // without this the sender simply never returns, and a test that
            // hangs stalls the run instead of reporting a failure
            self.writes += 1;
            assert!(
                self.writes < 50,
                "the sender has resent the same block {} times, it must give up",
                self.writes
            );
            self.outbox.push(NAK);
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

    #[test]
    fn a_receiver_that_never_accepts_anything_is_given_up_on() {
        let mut rx = Stubborn {
            outbox: vec![CRC],
            writes: 0,
        };
        let started = Instant::now();
        let err = send(&mut rx, "fw", &body(100), &quick(), |_| {}).expect_err("gives up");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "must not retry forever, took {:?}",
            started.elapsed()
        );
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn progress_is_reported_and_adds_up() {
        let data = body(5000);
        let mut seen = Vec::new();
        let mut rx = Receiver::new();

        send(&mut rx, "fw", &data, &quick(), |p| seen.push(p)).expect("sends");

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
        assert!(
            seen.windows(2).all(|w| w[0].sent <= w[1].sent),
            "progress must not go backwards"
        );
    }

    #[test]
    fn the_header_block_holds_the_name_and_the_size() {
        let block = header_block("hd1.bin", 12345);
        assert_eq!(&block[..7], b"hd1.bin");
        assert_eq!(block[7], 0, "the name is terminated");
        assert_eq!(&block[8..13], b"12345");
        assert!(block[13..].iter().all(|b| *b == 0), "the rest is padding");
    }

    #[test]
    fn a_long_name_cannot_crowd_out_the_size() {
        let block = header_block(&"x".repeat(500), 99);
        let name_end = block.iter().position(|b| *b == 0).expect("terminated");
        assert!(name_end <= SHORT_BLOCK - 18);

        let rest = &block[name_end + 1..];
        let size_end = rest.iter().position(|b| *b == 0).expect("terminated");
        assert_eq!(&rest[..size_end], b"99");
    }

    #[test]
    fn an_empty_file_still_completes_the_handshake() {
        let mut rx = Receiver::new();
        send(&mut rx, "empty", &[], &quick(), |_| {}).expect("sends");
        assert_eq!(rx.size, 0);
        assert!(rx.saved().is_empty());
        assert!(rx.finished);
    }
}
