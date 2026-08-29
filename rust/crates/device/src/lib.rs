//! Clone mode protocols for talking to amateur radio handsets.
//!
//! The protocols here are written against the [`ByteStream`] trait rather
//! than against a serial port, so the conversation with a radio can be
//! replayed against an in memory fake and checked byte for byte. That matters
//! more here than in the rest of this workspace: a firmware file can be
//! compared against a known good one, but a protocol mistake is only visible
//! when it is talking to a radio, and a radio that gets confused mid clone is
//! not always easy to recover.
//!
//! What is here is only the reading side of the clone protocols. Writing to a
//! radio is the part that can brick one, and it is not going in until it can
//! be tested against hardware.

pub mod ble;
pub mod dfu;
pub mod dfuse;
pub mod flash;
pub mod h8sx;
pub mod hid;
pub mod serial;
pub mod usb;
pub mod uv17pro;
pub mod uv5r;
pub mod xmodem;
pub mod ymodem;

use thiserror::Error;

/// Anything that can go wrong talking to a radio
#[derive(Debug, Error)]
pub enum Error {
    /// The radio said nothing, or not enough, before the deadline
    #[error("timed out reading {what}: wanted {wanted} bytes, got {got}")]
    Timeout {
        /// What was being read
        what: &'static str,
        /// How many bytes were expected
        wanted: usize,
        /// How many arrived
        got: usize,
    },

    /// The radio answered, but not with what the protocol expects
    #[error("{what}: expected {expected}, got {got}")]
    Unexpected {
        /// The step that went wrong
        what: &'static str,
        /// What should have arrived
        expected: String,
        /// What did
        got: String,
    },

    /// The radio never identified itself
    #[error("no response from radio, check the cable is seated and the radio is on")]
    NoRadio,

    /// The radio is not one this driver speaks to
    #[error("unknown radio model: {0}")]
    UnknownModel(String),

    /// The port itself failed
    #[error("serial port: {0}")]
    Port(String),

    /// A clone session can only run once
    #[error("this session has already been used, open the port again")]
    SessionSpent,
}

/// Result alias for this crate
pub type Result<T> = std::result::Result<T, Error>;

/// A boxed stream, so a caller can choose a transport at run time.
///
/// [`ByteStream`] has no generic methods and never returns `Self`, so it can
/// be used as a trait object. That is what keeps a driver from having to
/// know whether it is talking over a cable or over Bluetooth: without it,
/// every driver has to be instantiated once per transport at the call site.
pub type BoxedStream = Box<dyn ByteStream>;

impl<T: ByteStream + ?Sized> ByteStream for &mut T {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        (**self).write_all(data)
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>> {
        (**self).read(len)
    }

    fn flush_input(&mut self) -> Result<()> {
        (**self).flush_input()
    }

    fn sleep(&mut self, millis: u64) {
        (**self).sleep(millis);
    }
}

impl<T: ByteStream + ?Sized> ByteStream for Box<T> {
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        (**self).write_all(data)
    }

    fn read(&mut self, len: usize) -> Result<Vec<u8>> {
        (**self).read(len)
    }

    fn flush_input(&mut self) -> Result<()> {
        (**self).flush_input()
    }

    fn sleep(&mut self, millis: u64) {
        (**self).sleep(millis);
    }
}

/// A bidirectional stream of bytes to a radio.
///
/// Some radios clone over a serial cable and some over Bluetooth, but the
/// protocol on top is the same, so the drivers work against this.
pub trait ByteStream {
    /// Write every byte, or fail
    fn write_all(&mut self, data: &[u8]) -> Result<()>;

    /// Read up to `len` bytes, returning early if the radio stops talking
    fn read(&mut self, len: usize) -> Result<Vec<u8>>;

    /// Throw away anything already received but not yet read
    fn flush_input(&mut self) -> Result<()>;

    /// Wait, which some radios need between steps
    fn sleep(&mut self, millis: u64);

    /// Write one byte at a time with a pause between, because some radios
    /// drop the identify magic if it arrives in a single burst
    fn write_slowly(&mut self, data: &[u8], gap_millis: u64) -> Result<()> {
        for byte in data {
            self.write_all(&[*byte])?;
            self.sleep(gap_millis);
        }
        Ok(())
    }

    /// Read exactly `len` bytes or fail
    fn read_exact(&mut self, len: usize, what: &'static str) -> Result<Vec<u8>> {
        let got = self.read(len)?;
        if got.len() != len {
            return Err(Error::Timeout {
                what,
                wanted: len,
                got: got.len(),
            });
        }
        Ok(got)
    }
}
