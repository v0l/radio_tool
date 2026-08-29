//! Codeplug formats for amateur radio handsets.
//!
//! A codeplug is the channel memory: frequencies, names, tone squelch and the
//! per channel settings. This crate reads the image files, and like
//! `firmware` it touches no hardware, so everything here is a pure
//! function of its input.

pub mod rdt;
pub use uv5r::CHIRP_MAGIC;

pub mod uv17pro;
pub mod uv5r;

use thiserror::Error;

/// Anything that can go wrong reading a codeplug
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// The file is not one of the sizes this format comes in
    #[error("wrong image size: got {got} bytes, expected one of {expected:x?}")]
    WrongSize {
        /// The size of the file
        got: usize,
        /// The sizes this format is known to come in
        expected: &'static [usize],
    },

    /// The file is shorter than the structure it claims to hold
    #[error("image is truncated, {what} needs {wanted} bytes but the image holds {got}")]
    Truncated {
        /// The part of the image being read
        what: &'static str,
        /// How many bytes were needed
        wanted: usize,
        /// How many bytes the image holds
        got: usize,
    },

    /// The image is structurally wrong
    #[error("malformed codeplug: {0}")]
    Malformed(&'static str),
}

/// Result alias for this crate
pub type Result<T> = std::result::Result<T, Error>;

/// A codeplug that has been read and can be described.
///
/// This is deliberately narrow. The formats have almost nothing in common
/// beyond being readable and printable, and inventing a shared channel model
/// for them would mean flattening real differences between radios. What the
/// trait is for is dispatch: the command line should not have to know the
/// list of formats.
pub trait Codeplug: std::fmt::Display {
    /// Which format this is, for messages
    fn format(&self) -> &'static str;

    /// The radio this codeplug is for, as far as the file says
    fn radio(&self) -> String;
}

/// One entry in the list of formats that can be read.
///
/// Function pointers rather than a trait with a constructor, because a method
/// returning `Self` cannot be called through a trait object.
#[derive(Debug)]
pub struct Format {
    /// Name of the format
    pub name: &'static str,
    /// Cheap check of whether a file is this format
    pub is_supported: fn(&[u8]) -> bool,

    /// Read it
    pub parse: fn(&[u8]) -> Result<Box<dyn Codeplug>>,
}

/// Every format that can be read, in the order they are tried.
///
/// Order is not currently load bearing: a test in `tests/reference.rs`
/// asserts that each real image is claimed by exactly one format, and
/// reordering this list does not change any result. That is worth keeping
/// true. A format that recognises a file by its size alone, as the UV-17Pro
/// family does when a CHIRP image carries no model stamp, is one careless
/// change away from claiming files that are not its own.
pub const FORMATS: &[Format] = &[
    Format {
        name: "UV-5R",
        is_supported: uv5r::Uv5rCodeplug::is_supported,
        parse: |data| uv5r::Uv5rCodeplug::parse(data).map(|c| Box::new(c) as Box<dyn Codeplug>),
    },
    Format {
        name: "UV-17Pro",
        is_supported: uv17pro::Uv17ProCodeplug::is_supported,
        parse: |data| {
            uv17pro::Uv17ProCodeplug::parse(data).map(|c| Box::new(c) as Box<dyn Codeplug>)
        },
    },
    Format {
        name: "RDT",
        is_supported: rdt::RdtCodeplug::is_supported,
        parse: |data| rdt::RdtCodeplug::parse(data).map(|c| Box::new(c) as Box<dyn Codeplug>),
    },
];

/// Read a file as whichever format recognises it
pub fn identify(data: &[u8]) -> Option<&'static Format> {
    FORMATS.iter().find(|f| (f.is_supported)(data))
}

/// Read a file, or say that nothing recognises it
pub fn parse(data: &[u8]) -> Result<Box<dyn Codeplug>> {
    let format = identify(data).ok_or(Error::Malformed("not a codeplug this tool can read"))?;
    (format.parse)(data)
}
