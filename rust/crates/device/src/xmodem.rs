//! XMODEM, which is how OpenRTX hands over a radio's flash.
//!
//! A radio running OpenRTX appears as a USB serial port. Its Backup and
//! Restore menu drives an XMODEM transfer over that port, so reading a
//! codeplug out of one needs no bootloader and no DFU: the radio is simply
//! switched on.
//!
//! Only the CRC variant is implemented, because that is all OpenRTX offers:
//! its sender waits for `C` and will not accept the older checksum mode.
//!
//! The receiver drives the transfer:
//!
//! ```text
//! host                          radio
//!  -> C
//!                               <- block 1, 128 or 1024 bytes
//!  -> ACK
//!                               <- block 2 ...
//!  -> ACK after each
//!                               <- EOT
//!  -> ACK
//! ```
//!
//! A short final block is padded, by OpenRTX with `0x1a`, and nothing in the
//! protocol says how much of the last block is real. The caller is told the
//! whole thing and decides.

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
/// Block rejected, send it again
const NAK: u8 = 0x15;
/// Two in a row abort the transfer
const CAN: u8 = 0x18;
/// Asks the sender to start, in CRC mode
const CRC: u8 = b'C';

/// How a transfer behaves when the radio is slow or unhappy
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// How long to wait for the sender to start
    pub start_timeout: Duration,
    /// How long to wait for each block
    pub block_timeout: Duration,
    /// How many times to ask for a block again
    pub retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // the radio waits for a person to press PTT, so this is generous
            start_timeout: Duration::from_secs(60),
            block_timeout: Duration::from_secs(5),
            retries: 10,
        }
    }
}

/// Progress through a transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes received so far
    pub received: usize,
    /// Blocks received so far
    pub blocks: usize,
}

/// Receive a whole transfer.
///
/// Returns everything the sender sent, padding of the final block included,
/// because XMODEM carries no length and the caller knows better than this
/// does where the real data ends.
pub fn receive<S: ByteStream>(
    stream: &mut S,
    config: &Config,
    mut progress: impl FnMut(Progress),
) -> Result<Vec<u8>> {
    stream.flush_input()?;

    let mut data = Vec::new();
    let mut expected: u8 = 1;
    let mut started = false;
    let mut asks = 0usize;

    // the sender waits for C before it says anything
    stream.write_all(&[CRC])?;
    let deadline = Instant::now() + config.start_timeout;

    loop {
        let timeout = if started {
            config.block_timeout
        } else {
            Duration::from_secs(3)
        };

        let Some(first) = read_byte(stream, timeout)? else {
            if started {
                return Err(Error::Timeout {
                    what: "a block",
                    wanted: 1,
                    got: 0,
                });
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout {
                    what: "the start of a transfer, is the radio waiting for PTT?",
                    wanted: 1,
                    got: 0,
                });
            }
            // ask again, the sender may not have been ready
            stream.write_all(&[CRC])?;
            continue;
        };

        let block_size = match first {
            SOH => 128,
            STX => 1024,
            EOT => {
                stream.write_all(&[ACK])?;
                return Ok(data);
            }
            CAN => {
                return Err(Error::Unexpected {
                    what: "a block",
                    expected: "data".to_owned(),
                    got: "the radio cancelled the transfer".to_owned(),
                });
            }
            // anything else is noise on the line before the transfer starts
            _ => continue,
        };

        started = true;

        // block number, its complement, the data, then a two byte CRC
        let header = stream.read_exact(2, "block number")?;
        let number = header.first().copied().unwrap_or(0);
        let complement = header.get(1).copied().unwrap_or(0);

        let body = stream.read_exact(block_size, "block")?;
        let checksum = stream.read_exact(2, "block checksum")?;
        let sent = u16::from_be_bytes([
            checksum.first().copied().unwrap_or(0),
            checksum.get(1).copied().unwrap_or(0),
        ]);

        let good = number == !complement && crc16(&body) == sent;

        if !good {
            asks += 1;
            if asks >= config.retries {
                return Err(Error::Unexpected {
                    what: "a block",
                    expected: "a block that arrives intact".to_owned(),
                    got: format!("{asks} bad blocks in a row"),
                });
            }
            stream.write_all(&[NAK])?;
            continue;
        }

        if number == expected.wrapping_sub(1) {
            // the sender did not hear the last acknowledgement and has sent
            // the block again: take it, but do not store it twice
            stream.write_all(&[ACK])?;
            continue;
        }

        if number != expected {
            return Err(Error::Unexpected {
                what: "block number",
                expected: format!("{expected}"),
                got: format!("{number}, so blocks have been lost"),
            });
        }

        data.extend_from_slice(&body);
        expected = expected.wrapping_add(1);
        asks = 0;
        stream.write_all(&[ACK])?;

        progress(Progress {
            received: data.len(),
            blocks: data.len().div_ceil(block_size),
        });
    }
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

/// The CRC-16 that XMODEM uses: polynomial 0x1021, starting at zero
pub fn crc16(data: &[u8]) -> u16 {
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

    /// A sender that behaves the way OpenRTX's does: waits for C, sends
    /// blocks numbered from one, pads a short final block with 0x1a
    struct Radio {
        outgoing: Vec<u8>,
        pub acks: usize,
        pub naks: usize,
        data: Vec<u8>,
        block_size: usize,
        started: bool,
        next: u8,
        sent: usize,
        /// Corrupt this block number once, to exercise the retry
        pub corrupt: Option<u8>,
        corrupted: bool,
    }

    impl Radio {
        fn new(data: Vec<u8>, block_size: usize) -> Self {
            Self {
                outgoing: Vec::new(),
                acks: 0,
                naks: 0,
                data,
                block_size,
                started: false,
                next: 1,
                sent: 0,
                corrupt: None,
                corrupted: false,
            }
        }

        fn queue_block(&mut self) {
            if self.sent >= self.data.len() {
                self.outgoing.push(EOT);
                return;
            }

            let end = (self.sent + self.block_size).min(self.data.len());
            let mut body = self.data[self.sent..end].to_vec();
            // OpenRTX pads a short block with 0x1a
            body.resize(self.block_size, 0x1a);

            let start = if self.block_size == 128 { SOH } else { STX };
            self.outgoing.push(start);
            self.outgoing.push(self.next);
            self.outgoing.push(!self.next);
            self.outgoing.extend_from_slice(&body);

            let mut crc = crc16(&body);
            if self.corrupt == Some(self.next) && !self.corrupted {
                crc ^= 0xffff;
                self.corrupted = true;
                // a corrupted block is not counted as delivered
                self.outgoing.extend_from_slice(&crc.to_be_bytes());
                return;
            }

            self.outgoing.extend_from_slice(&crc.to_be_bytes());
            self.sent = end;
            self.next = self.next.wrapping_add(1);
        }
    }

    impl ByteStream for Radio {
        fn write_all(&mut self, data: &[u8]) -> Result<()> {
            for byte in data {
                match *byte {
                    CRC if !self.started => {
                        self.started = true;
                        self.queue_block();
                    }
                    CRC => {}
                    ACK => {
                        self.acks += 1;
                        self.queue_block();
                    }
                    NAK => {
                        self.naks += 1;
                        self.queue_block();
                    }
                    _ => {}
                }
            }
            Ok(())
        }

        fn read(&mut self, len: usize) -> Result<Vec<u8>> {
            let take = len.min(self.outgoing.len());
            Ok(self.outgoing.drain(..take).collect())
        }

        fn flush_input(&mut self) -> Result<()> {
            Ok(())
        }

        fn sleep(&mut self, _millis: u64) {}
    }

    fn quick() -> Config {
        Config {
            start_timeout: Duration::from_millis(200),
            block_timeout: Duration::from_millis(200),
            retries: 10,
        }
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn the_crc_matches_the_known_answer() {
        // the check value every XMODEM implementation is tested against
        assert_eq!(crc16(b"123456789"), 0x31C3);
    }

    #[test]
    fn a_transfer_arrives_intact() {
        for (len, block) in [(1024, 1024), (4096, 1024), (256, 128), (128, 128)] {
            let data = body(len);
            let mut radio = Radio::new(data.clone(), block);

            let got = receive(&mut radio, &quick(), |_| {})
                .unwrap_or_else(|e| panic!("{len} bytes in {block} byte blocks: {e}"));

            assert_eq!(got, data, "{len} bytes did not arrive intact");
        }
    }

    #[test]
    fn a_short_final_block_arrives_padded_and_is_the_callers_problem() {
        // 1500 bytes is one full block and a short one
        let data = body(1500);
        let mut radio = Radio::new(data.clone(), 1024);

        let got = receive(&mut radio, &quick(), |_| {}).expect("receives");

        assert_eq!(got.len(), 2048, "the padding comes with it");
        assert_eq!(&got[..1500], &data[..], "and the data is intact underneath");
        assert!(
            got[1500..].iter().all(|b| *b == 0x1a),
            "OpenRTX pads with 0x1a"
        );
    }

    #[test]
    fn a_corrupt_block_is_asked_for_again() {
        let data = body(4096);
        let mut radio = Radio::new(data.clone(), 1024);
        radio.corrupt = Some(2);

        let got = receive(&mut radio, &quick(), |_| {}).expect("recovers");
        assert_eq!(got, data);
        assert_eq!(radio.naks, 1, "exactly one block was asked for again");
    }

    #[test]
    fn a_radio_that_never_starts_says_what_to_do() {
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

        let err = receive(&mut Silent, &quick(), |_| {}).expect_err("times out");
        assert!(
            err.to_string().contains("PTT"),
            "the message should help: {err}"
        );
    }

    #[test]
    fn a_cancelled_transfer_is_not_mistaken_for_a_short_one() {
        struct Canceller {
            sent: bool,
        }
        impl ByteStream for Canceller {
            fn write_all(&mut self, _data: &[u8]) -> Result<()> {
                Ok(())
            }
            fn read(&mut self, _len: usize) -> Result<Vec<u8>> {
                if self.sent {
                    return Ok(Vec::new());
                }
                self.sent = true;
                Ok(vec![CAN])
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _millis: u64) {}
        }

        let err = receive(&mut Canceller { sent: false }, &quick(), |_| {})
            .expect_err("a cancelled transfer is not a result");
        assert!(err.to_string().contains("cancel"), "{err}");
    }

    /// A sender that misses an acknowledgement repeats the block. Storing it
    /// twice would shift everything after it.
    #[test]
    fn a_repeated_block_is_acknowledged_but_not_stored_twice() {
        struct Repeater {
            outgoing: Vec<u8>,
            stage: usize,
        }
        impl Repeater {
            fn block(&mut self, number: u8, fill: u8) {
                let body = vec![fill; 128];
                self.outgoing.push(SOH);
                self.outgoing.push(number);
                self.outgoing.push(!number);
                self.outgoing.extend_from_slice(&body);
                self.outgoing.extend_from_slice(&crc16(&body).to_be_bytes());
            }
        }
        impl ByteStream for Repeater {
            fn write_all(&mut self, data: &[u8]) -> Result<()> {
                for byte in data {
                    if *byte == CRC || *byte == ACK {
                        self.stage += 1;
                        match self.stage {
                            1 => self.block(1, 0xaa),
                            // the same block again, as if the ack was missed
                            2 => self.block(1, 0xaa),
                            3 => self.block(2, 0xbb),
                            _ => self.outgoing.push(EOT),
                        }
                    }
                }
                Ok(())
            }
            fn read(&mut self, len: usize) -> Result<Vec<u8>> {
                let take = len.min(self.outgoing.len());
                Ok(self.outgoing.drain(..take).collect())
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _millis: u64) {}
        }

        let got = receive(
            &mut Repeater {
                outgoing: Vec::new(),
                stage: 0,
            },
            &quick(),
            |_| {},
        )
        .expect("receives");

        assert_eq!(got.len(), 256, "the repeat was not stored twice");
        assert!(got[..128].iter().all(|b| *b == 0xaa));
        assert!(got[128..].iter().all(|b| *b == 0xbb));
    }

    #[test]
    fn a_block_out_of_sequence_stops_the_transfer() {
        struct Skipper {
            outgoing: Vec<u8>,
            stage: usize,
        }
        impl ByteStream for Skipper {
            fn write_all(&mut self, data: &[u8]) -> Result<()> {
                for byte in data {
                    if *byte == CRC || *byte == ACK {
                        self.stage += 1;
                        // block 1, then block 3: block 2 was lost
                        let number = if self.stage == 1 { 1u8 } else { 3u8 };
                        let body = vec![0u8; 128];
                        self.outgoing.push(SOH);
                        self.outgoing.push(number);
                        self.outgoing.push(!number);
                        self.outgoing.extend_from_slice(&body);
                        self.outgoing.extend_from_slice(&crc16(&body).to_be_bytes());
                    }
                }
                Ok(())
            }
            fn read(&mut self, len: usize) -> Result<Vec<u8>> {
                let take = len.min(self.outgoing.len());
                Ok(self.outgoing.drain(..take).collect())
            }
            fn flush_input(&mut self) -> Result<()> {
                Ok(())
            }
            fn sleep(&mut self, _millis: u64) {}
        }

        let err = receive(
            &mut Skipper {
                outgoing: Vec::new(),
                stage: 0,
            },
            &quick(),
            |_| {},
        )
        .expect_err("a gap is not something to paper over");
        assert!(err.to_string().contains("lost"), "{err}");
    }
}
