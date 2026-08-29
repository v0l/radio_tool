//! The Baofeng UV-5R family codeplug image.
//!
//! An image is an eight byte ident block followed by the radio memory, so
//! every offset here is a file offset and matches CHIRP's `#seekto` values
//! directly.
//!
//! The layout, the bit positions and the DTCS table were all cross checked
//! against CHIRP's `chirp/drivers/uv5r.py`, which is the reference for this
//! family and is exercised by far more radios than this crate will ever see.
//! CHIRP writes its bitfields most significant first, which is why the masks
//! below look reversed compared to the field order in its `MEM_FORMAT`.

use crate::{Error, Result};
use std::fmt;

/// Eight byte ident block at the start of an image
pub const IDENT_LEN: usize = 0x08;
/// Channels start here, `#seekto 0x0008` in CHIRP
pub const CHANNELS_OFFSET: usize = 0x0008;
/// Channel names, `#seekto 0x1008`
pub const NAMES_OFFSET: usize = 0x1008;
/// Power on message, CHIRP's `_mem_params`
pub const POWER_ON_MSG_OFFSET: usize = 0x1828;
/// Firmware version, CHIRP's `_fw_ver_file_start`
pub const FIRMWARE_OFFSET: usize = 0x1838;

/// Channels in an image
pub const CHANNEL_COUNT: usize = 128;
/// Bytes per channel, and per name
const CHANNEL_LEN: usize = 0x10;
/// Characters in a channel name
const NAME_LEN: usize = 7;
/// Characters per line in the firmware and power on strings
const LINE_LEN: usize = 7;

/// Image sizes CHIRP accepts for this family: memory only, memory plus the
/// aux block, and that plus an eight byte model id
const IMAGE_SIZES: [usize; 3] = [0x1808, 0x1948, 0x1950];

/// CHIRP appends this and a base64 metadata blob when it saves a `.img`.
/// Taken from `chirp_common.py`, where it is `FileBackedRadio.MAGIC`.
/// The marker CHIRP puts before its metadata trailer
pub const CHIRP_MAGIC: &[u8] = b"\x00\xffchirp\xeeimg\x00\x01";

/// Where the eight byte model id lives in a 0x1950 image
const MODEL_ID_OFFSET: usize = 0x1948;

/// Firmware version prefixes that mark an image as belonging to this family.
/// This is CHIRP's `BASETYPE_LIST`, the union of every variant it supports.
const BASE_TYPES: [&[u8]; 26] = [
    b"BFS",
    b"BFB",
    b"N5R-2",
    b"N5R2",
    b"N5RV",
    b"BTS",
    b"D5R2",
    b"B5R2", // UV-5R
    b"USA",  // F-11
    b"US2S2",
    b"B82S",
    b"BF82",
    b"N82-2",
    b"N822", // UV-82
    b"BJ55", // Baojie UV-55
    b"BF1",
    b"UV6",      // UV-6
    b"BFP3V3 B", // KT980HP
    b"BFP3V3 F",
    b"N5R-3",
    b"N5R3",
    b"F5R3",
    b"BFT", // F8HP
    b"N82-3",
    b"N823",    // UV-82HP
    b"HN5RV01", // UV-82X3
];

/// Strip the metadata CHIRP appends when it saves an image.
///
/// A file straight off a radio has none. One saved by CHIRP carries a magic,
/// then base64 JSON describing the driver that wrote it. Returns the input
/// unchanged when there is no trailer.
pub fn strip_chirp_metadata(input: &[u8]) -> &[u8] {
    input
        .windows(CHIRP_MAGIC.len())
        .position(|w| w == CHIRP_MAGIC)
        .and_then(|at| input.get(..at))
        .unwrap_or(input)
}

/// Does the firmware string mark this as a radio in this family
fn has_known_base_type(image: &[u8]) -> bool {
    let Some(field) = image.get(FIRMWARE_OFFSET..FIRMWARE_OFFSET + (LINE_LEN * 2)) else {
        return false;
    };
    BASE_TYPES
        .iter()
        .any(|base| field.windows(base.len()).any(|w| w == *base))
}

/// Tones at or above this raw value are CTCSS, below are DTCS
const CTCSS_FLOOR: u16 = 0x0258;
/// Raw DTCS values above this are the inverted set
const DTCS_INVERT_FLOOR: u16 = 0x69;

/// `sorted(chirp_common.DTCS_CODES + (645,))`, which is CHIRP's `UV5R_DTCS`.
/// Extracted from CHIRP and checked to be identical, value for value and in
/// the same order, by `tools/check_dtcs.py`.
pub const DTCS_CODES: [u16; 105] = [
    23, 25, 26, 31, 32, 36, 43, 47, 51, 53, 54, 65, 71, 72, 73, 74, 114, 115, 116, 122, 125, 131,
    132, 134, 143, 145, 152, 155, 156, 162, 165, 172, 174, 205, 212, 223, 225, 226, 243, 244, 245,
    246, 251, 252, 255, 261, 263, 265, 266, 271, 274, 306, 311, 315, 325, 331, 332, 343, 346, 351,
    356, 364, 365, 371, 411, 412, 413, 423, 431, 432, 445, 446, 452, 454, 455, 462, 464, 465, 466,
    503, 506, 516, 523, 526, 532, 546, 565, 606, 612, 624, 627, 631, 632, 645, 654, 662, 664, 703,
    712, 723, 731, 732, 734, 743, 754,
];

/// A CTCSS or DTCS tone, or none
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// No tone squelch
    None,
    /// CTCSS, held in tenths of a Hz
    Ctcss {
        /// Frequency in tenths of a Hz, so 885 is 88.5 Hz
        tenths: u16,
    },
    /// DTCS, also called DCS
    Dtcs {
        /// The code itself, from [`DTCS_CODES`]
        code: u16,
        /// The inverted set, shown as R rather than N
        inverted: bool,
    },
    /// A raw value that is not a tone this radio can mean
    Unknown {
        /// The value as stored
        raw: u16,
    },
}

impl Tone {
    /// Decode a raw tone field, following CHIRP's `_get_tone`
    pub fn from_raw(raw: u16) -> Self {
        if raw == 0 || raw == 0xffff {
            return Self::None;
        }
        if raw >= CTCSS_FLOOR {
            return Self::Ctcss { tenths: raw };
        }

        let (index, inverted) = if raw > DTCS_INVERT_FLOOR {
            (raw - (DTCS_INVERT_FLOOR + 1), true)
        } else {
            (raw - 1, false)
        };

        match DTCS_CODES.get(usize::from(index)) {
            Some(code) => Self::Dtcs {
                code: *code,
                inverted,
            },
            None => Self::Unknown { raw },
        }
    }
}

impl fmt::Display for Tone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Ctcss { tenths } => write!(f, "{}.{}", tenths / 10, tenths % 10),
            Self::Dtcs { code, inverted } => {
                write!(f, "D{:03}{}", code, if *inverted { 'R' } else { 'N' })
            }
            Self::Unknown { raw } => write!(f, "?{raw:#06x}"),
        }
    }
}

/// Transmit power, two levels on most of this family
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Full power
    High,
    /// Reduced power
    Low,
}

impl fmt::Display for Power {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "High"),
            Self::Low => write!(f, "Low"),
        }
    }
}

/// One memory channel
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    /// Receive frequency in Hz
    pub rx_freq: u32,
    /// Transmit frequency in Hz, meaningless when [`Self::tx_inhibit`] is set
    pub tx_freq: u32,
    /// The channel is receive only
    pub tx_inhibit: bool,
    /// Channel name, up to seven characters
    pub name: String,
    /// Tone squelch on receive
    pub rx_tone: Tone,
    /// Tone sent while transmitting
    pub tx_tone: Tone,
    /// Wide, 25 kHz, rather than narrow, 12.5 kHz
    pub wide: bool,
    /// Included in scans
    pub scan: bool,
    /// Busy channel lockout
    pub bcl: bool,
    /// Transmit power
    pub power: Power,
    /// PTT ID mode
    pub pttid: u8,
    /// Signalling code index
    pub scode: u8,
}

/// A parsed codeplug image
#[derive(Debug, Clone)]
pub struct Uv5rCodeplug {
    data: Vec<u8>,
    channels: Vec<Option<Channel>>,
    firmware_version: String,
    power_on_msg: String,
}

impl Uv5rCodeplug {
    /// Every channel slot, empty ones included
    pub fn channels(&self) -> &[Option<Channel>] {
        &self.channels
    }

    /// Only the slots that hold a channel, with their positions
    pub fn used_channels(&self) -> impl Iterator<Item = (usize, &Channel)> {
        self.channels
            .iter()
            .enumerate()
            .filter_map(|(ix, ch)| ch.as_ref().map(|c| (ix, c)))
    }

    /// The firmware version string the radio reported
    pub fn firmware_version(&self) -> &str {
        &self.firmware_version
    }

    /// The two line power on message, joined with a slash
    pub fn power_on_message(&self) -> &str {
        &self.power_on_msg
    }

    /// The image as it was read
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Does this look like a UV-5R image.
    ///
    /// Identification follows CHIRP's `model_match`: the size has to be one
    /// this family comes in, and either the trailing model id or the firmware
    /// version string has to name a radio we know. A real radio returns an
    /// ident like `\xaaBFB231\xdd`, so the ident block is a weak signal and is
    /// only used as a fallback.
    pub fn is_supported(input: &[u8]) -> bool {
        let image = strip_chirp_metadata(input);
        if !IMAGE_SIZES.contains(&image.len()) {
            return false;
        }

        // a 0x1950 image carries the model id in the last eight bytes
        if image.len() == 0x1950 {
            if let Some(id) = image.get(MODEL_ID_OFFSET..) {
                if id.iter().any(|c| c.is_ascii_uppercase()) {
                    return true;
                }
            }
        }

        if has_known_base_type(image) {
            return true;
        }

        // images written by radio_tool against a radio that echoes the magic
        matches!(image.get(..3), Some([0x50, 0xbb, 0xff]))
    }

    /// Read an image, ignoring any metadata CHIRP appended to it
    pub fn parse(input: &[u8]) -> Result<Self> {
        let input = strip_chirp_metadata(input);

        if !IMAGE_SIZES.contains(&input.len()) {
            return Err(Error::WrongSize {
                got: input.len(),
                expected: &IMAGE_SIZES,
            });
        }

        let needed = NAMES_OFFSET + (CHANNEL_COUNT * CHANNEL_LEN);
        if input.len() < needed {
            return Err(Error::Truncated {
                what: "channel names",
                wanted: needed,
                got: input.len(),
            });
        }

        let mut channels = Vec::with_capacity(CHANNEL_COUNT);
        for ix in 0..CHANNEL_COUNT {
            channels.push(parse_channel(input, ix)?);
        }

        let firmware_version =
            read_string(input, FIRMWARE_OFFSET, LINE_LEN * 2).unwrap_or_default();

        let line1 = read_string(input, POWER_ON_MSG_OFFSET, LINE_LEN).unwrap_or_default();
        let line2 =
            read_string(input, POWER_ON_MSG_OFFSET + LINE_LEN, LINE_LEN).unwrap_or_default();
        let power_on_msg = match (line1.is_empty(), line2.is_empty()) {
            (true, true) => String::new(),
            (false, true) => line1,
            (true, false) => line2,
            (false, false) => format!("{line1} / {line2}"),
        };

        Ok(Self {
            data: input.to_vec(),
            channels,
            firmware_version,
            power_on_msg,
        })
    }
}

impl fmt::Display for Uv5rCodeplug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, " == Baofeng UV-5R Codeplug ==")?;
        writeln!(
            f,
            "Firmware: {}",
            if self.firmware_version.is_empty() {
                "Unknown"
            } else {
                &self.firmware_version
            }
        )?;
        writeln!(
            f,
            "Power on: {}",
            if self.power_on_msg.is_empty() {
                "(none)"
            } else {
                &self.power_on_msg
            }
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "  # Name       RX Freq    TX Freq    RX Tone  TX Tone  BW      Pwr   Scan"
        )?;

        let mut used = 0;
        for (ix, ch) in self.used_channels() {
            used += 1;
            let tx = if ch.tx_inhibit {
                "off".to_owned()
            } else {
                format_freq(ch.tx_freq)
            };
            writeln!(
                f,
                "{:3} {:<10} {:>10} {:>10} {:>8} {:>8} {:<7} {:<5} {}",
                ix,
                ch.name,
                format_freq(ch.rx_freq),
                tx,
                ch.rx_tone.to_string(),
                ch.tx_tone.to_string(),
                if ch.wide { "Wide" } else { "Narrow" },
                ch.power.to_string(),
                if ch.scan { "Yes" } else { "No" }
            )?;
        }

        writeln!(f)?;
        write!(f, "{} of {} channels used", used, self.channels.len())
    }
}

/// Read one channel, returning None for an empty slot
fn parse_channel(input: &[u8], index: usize) -> Result<Option<Channel>> {
    let at = CHANNELS_OFFSET + (index * CHANNEL_LEN);
    let mem = input
        .get(at..at + CHANNEL_LEN)
        .ok_or(Error::Malformed("channel record"))?;

    // an unprogrammed slot is all ones
    if mem.first() == Some(&0xff) {
        return Ok(None);
    }

    let rx_freq = read_bcd_freq(mem, 0)?;
    let tx_inhibit = matches!(mem.get(4..8), Some([0xff, 0xff, 0xff, 0xff]));
    let tx_freq = if tx_inhibit {
        0
    } else {
        read_bcd_freq(mem, 4)?
    };

    let rx_tone = Tone::from_raw(read_u16(mem, 8)?);
    let tx_tone = Tone::from_raw(read_u16(mem, 10)?);

    // CHIRP writes these bitfields most significant first:
    //   byte 12: unused1:3, isuhf:1, scode:4
    //   byte 14: mailicon:3, unknown2:3, lowpower:2
    //   byte 15: unknown3:1, wide:1, unknown4:2, bcl:1, scan:1, pttid:2
    let scode = mem.get(12).ok_or(Error::Malformed("channel scode"))? & 0x0f;
    let power_bits = mem.get(14).ok_or(Error::Malformed("channel power"))? & 0x03;
    let flags = *mem.get(15).ok_or(Error::Malformed("channel flags"))?;

    let name_at = NAMES_OFFSET + (index * CHANNEL_LEN);
    let name = read_string(input, name_at, NAME_LEN).unwrap_or_default();

    Ok(Some(Channel {
        rx_freq,
        tx_freq,
        tx_inhibit,
        name,
        rx_tone,
        tx_tone,
        wide: flags & 0x40 != 0,
        scan: flags & 0x04 != 0,
        bcl: flags & 0x08 != 0,
        power: if power_bits == 0 {
            Power::High
        } else {
            Power::Low
        },
        pttid: flags & 0x03,
        scode,
    }))
}

/// Little endian BCD frequency, stored in units of 10 Hz
fn read_bcd_freq(buf: &[u8], offset: usize) -> Result<u32> {
    let field = buf
        .get(offset..offset + 4)
        .ok_or(Error::Malformed("frequency"))?;

    let mut value: u32 = 0;
    for byte in field.iter().rev() {
        let hi = u32::from(byte >> 4);
        let lo = u32::from(byte & 0x0f);
        if hi > 9 || lo > 9 {
            // not valid BCD, an unprogrammed or corrupt slot
            return Ok(0);
        }
        value = value * 100 + (hi * 10) + lo;
    }
    Ok(value.saturating_mul(10))
}

fn read_u16(buf: &[u8], offset: usize) -> Result<u16> {
    let bytes = buf
        .get(offset..offset + 2)
        .ok_or(Error::Malformed("16 bit field"))?;
    Ok(u16::from_le_bytes([
        *bytes.first().ok_or(Error::Malformed("16 bit field"))?,
        *bytes.get(1).ok_or(Error::Malformed("16 bit field"))?,
    ]))
}

/// A fixed width string, stopping at a null, with 0xff treated as a space
/// because the vendor software pads with it, sometimes mid name
fn read_string(buf: &[u8], offset: usize, len: usize) -> Option<String> {
    let field = buf.get(offset..offset + len)?;
    let mut out = String::with_capacity(len);
    for byte in field {
        match byte {
            0x00 => break,
            0xff => out.push(' '),
            c if c.is_ascii_graphic() || *c == b' ' => out.push(char::from(*c)),
            _ => out.push('.'),
        }
    }
    Some(out.trim_end().to_owned())
}

fn format_freq(hz: u32) -> String {
    format!("{}.{:05}", hz / 1_000_000, (hz % 1_000_000) / 10)
}

impl crate::Codeplug for Uv5rCodeplug {
    fn format(&self) -> &'static str {
        "UV-5R"
    }

    fn radio(&self) -> String {
        "UV-5R".to_owned()
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

    #[test]
    fn dtcs_table_is_sorted_and_holds_645() {
        // CHIRP builds it as sorted(DTCS_CODES + (645,))
        let mut sorted = DTCS_CODES;
        sorted.sort_unstable();
        assert_eq!(sorted, DTCS_CODES, "the table must be in ascending order");
        assert!(DTCS_CODES.contains(&645), "645 is the UV-5R addition");
        assert_eq!(DTCS_CODES.len(), 105);
    }

    #[test]
    fn tone_decoding_follows_chirp() {
        assert_eq!(Tone::from_raw(0), Tone::None);
        assert_eq!(Tone::from_raw(0xffff), Tone::None);

        // at and above 0x0258 is CTCSS, in tenths of a Hz
        assert_eq!(Tone::from_raw(0x0258), Tone::Ctcss { tenths: 600 });
        assert_eq!(Tone::from_raw(885), Tone::Ctcss { tenths: 885 });
        assert_eq!(Tone::from_raw(885).to_string(), "88.5");
        assert_eq!(Tone::from_raw(1000).to_string(), "100.0");

        // 1 is the first DTCS code, not inverted
        assert_eq!(
            Tone::from_raw(1),
            Tone::Dtcs {
                code: 23,
                inverted: false
            }
        );
        assert_eq!(Tone::from_raw(1).to_string(), "D023N");

        // 0x69 is the last of the normal set, 0x6A the first inverted one
        assert_eq!(
            Tone::from_raw(0x69),
            Tone::Dtcs {
                code: DTCS_CODES[0x68],
                inverted: false
            }
        );
        assert_eq!(
            Tone::from_raw(0x6a),
            Tone::Dtcs {
                code: 23,
                inverted: true
            }
        );
        assert_eq!(Tone::from_raw(0x6a).to_string(), "D023R");

        // the last inverted code, and one past it
        let last = 0x6a + (DTCS_CODES.len() as u16) - 1;
        assert!(matches!(Tone::from_raw(last), Tone::Dtcs { .. }));
        assert_eq!(Tone::from_raw(last + 1), Tone::Unknown { raw: last + 1 });
    }

    #[test]
    fn every_raw_tone_value_decodes_without_panicking() {
        for raw in 0..=u16::MAX {
            let _ = Tone::from_raw(raw).to_string();
        }
    }

    #[test]
    fn bcd_frequencies_are_little_endian_tens_of_hz() {
        // these byte patterns are taken from a real image, not invented:
        // the value is held in units of 10 Hz, least significant pair first
        assert_eq!(
            read_bcd_freq(&[0x00, 0x00, 0x55, 0x14], 0).unwrap(),
            145_500_000,
            "145.50000 MHz"
        );
        assert_eq!(
            read_bcd_freq(&[0x00, 0x25, 0x56, 0x14], 0).unwrap(),
            145_625_000,
            "145.62500 MHz"
        );
        assert_eq!(
            read_bcd_freq(&[0x25, 0x06, 0x60, 0x44], 0).unwrap(),
            446_006_250,
            "446.00625 MHz"
        );
        // an unprogrammed slot is not valid BCD
        assert_eq!(read_bcd_freq(&[0xff, 0xff, 0xff, 0xff], 0).unwrap(), 0);
    }

    #[test]
    fn frequencies_format_to_five_decimal_places() {
        assert_eq!(format_freq(145_500_000), "145.50000");
        assert_eq!(format_freq(446_006_250), "446.00625");
    }

    #[test]
    fn names_stop_at_a_null_and_treat_ff_as_a_space() {
        assert_eq!(read_string(b"ABC\0XYZ", 0, 7).unwrap(), "ABC");
        assert_eq!(read_string(b"AB\xffCD\xff\xff", 0, 7).unwrap(), "AB CD");
        assert_eq!(
            read_string(b"\xff\xff\xff\xff\xff\xff\xff", 0, 7).unwrap(),
            ""
        );
    }

    #[test]
    fn wrong_sizes_are_rejected() {
        for size in [0usize, 1, 0x1807, 0x1809, 0x1949] {
            let data = vec![0x50; size];
            assert!(!Uv5rCodeplug::is_supported(&data), "{size:#x} was accepted");
            assert!(matches!(
                Uv5rCodeplug::parse(&data),
                Err(Error::WrongSize { .. })
            ));
        }
    }

    #[test]
    fn a_wrong_ident_is_not_supported() {
        let mut data = vec![0u8; 0x1948];
        data[..3].copy_from_slice(&[0x50, 0xbb, 0xff]);
        assert!(Uv5rCodeplug::is_supported(&data));

        data[1] = 0xaa;
        assert!(!Uv5rCodeplug::is_supported(&data));
    }

    #[test]
    fn an_empty_image_parses_with_no_channels() {
        let mut data = vec![0xffu8; 0x1948];
        data[..3].copy_from_slice(&[0x50, 0xbb, 0xff]);

        let cp = Uv5rCodeplug::parse(&data).expect("parses");
        assert_eq!(cp.channels().len(), CHANNEL_COUNT);
        assert_eq!(cp.used_channels().count(), 0);
    }

    #[test]
    fn arbitrary_content_never_panics() {
        // only the sizes we accept can get past parse, so fuzz those
        for size in IMAGE_SIZES {
            let mut x: u32 = 12345;
            let data: Vec<u8> = (0..size)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    (x & 0xff) as u8
                })
                .collect();
            if let Ok(cp) = Uv5rCodeplug::parse(&data) {
                let _ = cp.to_string();
            }
        }
    }
}
