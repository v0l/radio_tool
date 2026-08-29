//! Firmware container formats for amateur radio handsets.
//!
//! This crate is a port of the firmware handling in the C++ radio_tool, and
//! deliberately covers only file parsing and generation: no USB, no serial, no
//! device access. Everything here is a pure function of its input, so it can
//! be checked byte for byte against the C++ implementation.

pub mod ailunce;
pub mod cipher;
pub mod cs;
pub mod keyguess;
pub mod sgl;
pub mod tyt;
pub mod yaesu;

use thiserror::Error;

/// One contiguous region of firmware and the address it is written to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment<'a> {
    /// Address on the device
    pub address: u32,
    /// Decrypted firmware data
    pub data: &'a [u8],
}

/// Anything that can go wrong reading or writing a firmware file
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// The file is shorter than the structure it claims to hold
    #[error("file is truncated, {what} needs {wanted} bytes but the file holds {got}")]
    Truncated {
        /// The part of the file that was being read
        what: &'static str,
        /// How many bytes were needed
        wanted: usize,
        /// How many bytes the file actually holds
        got: usize,
    },

    /// The start magic is not one this container recognises
    #[error("not a firmware file for this container, the start magic does not match")]
    BadMagic,

    /// The header is structurally wrong
    #[error("malformed firmware header: {0}")]
    Malformed(&'static str),

    /// The counter magic does not belong to any radio we know
    #[error("counter magic does not match any supported radio")]
    UnsupportedCounterMagic,

    /// No radio goes by that name
    #[error("unknown radio model: {0}")]
    UnknownModel(String),

    /// The region table is a fixed size and this many regions will not fit
    #[error("too many memory regions: {0}")]
    TooManyRegions(usize),

    /// A single region is larger than the format can describe
    #[error("segment is too large for the format: {0} bytes")]
    SegmentTooLarge(usize),

    /// Nothing to write
    #[error("no firmware segments to write")]
    NoSegments,

    /// The checksum in the file does not cover the data that follows it
    #[error("checksum mismatch, the file holds {stored:#06x} but the data sums to {computed:#06x}")]
    BadChecksum {
        /// The checksum the file carries
        stored: u16,
        /// The checksum the data actually produces
        computed: u16,
    },
}

/// Result alias for this crate
pub type Result<T> = std::result::Result<T, Error>;
