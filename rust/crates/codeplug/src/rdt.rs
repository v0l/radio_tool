//! The TYT RDT codeplug, used by the MD-380 family, MD-2017, MD-UV3x0 and
//! the Baofeng DM-1701.
//!
//! An RDT file is a 0x225 byte header, then the radio memory image, with a
//! 0x10 byte footer partway through on the larger radios. Everything this
//! module reads lives in the first part of the memory image, so the footer
//! does not come into it.
//!
//! ```text
//! 0x000    5  "DfuSe"
//! 0x005    1  unknown
//! 0x006    4  channel offset
//! 0x00a    1  unknown
//! 0x00b    6  "Target"
//! 0x011    1  unknown
//! 0x012    4  unknown
//! 0x016  255  target name
//! 0x115   16  four unknown u32
//! 0x125   16  radio model
//! 0x135  240  unknown
//! 0x225    n  radio memory, addressed from zero
//! ```
//!
//! Offsets inside the memory image:
//!
//! ```text
//! 0x2001  11  timestamp, then the CPS software version
//! 0x2040 144  general settings
//! ```
//!
//! Every offset and the whole settings layout were cross checked against
//! [dmrconfig](https://github.com/sergev/dmrconfig), which reverse engineered
//! these radios separately, and they agree throughout. dmrconfig also decodes
//! the four version characters after the timestamp, which radio_tool skips,
//! so this port reads them too.

use crate::{Error, Result};
use std::fmt;

/// Size of the RDT header, before the memory image
pub const HEADER_LEN: usize = 0x225;

/// Timestamp block, inside the memory image
const TIMESTAMP_OFFSET: usize = 0x2001;
/// Seven BCD bytes of date and time, then four of version
const TIMESTAMP_LEN: usize = 7;
const CPS_VERSION_LEN: usize = 4;

/// General settings block, inside the memory image
const SETTINGS_OFFSET: usize = 0x2040;
/// Size of that block
const SETTINGS_LEN: usize = 0x90;

const MAGIC: &[u8] = b"DfuSe";
const TARGET: &[u8] = b"Target";

/// Header field offsets
const CHANNEL_OFFSET_AT: usize = 0x06;
const TARGET_AT: usize = 0x0b;
const TARGET_NAME_AT: usize = 0x16;
const TARGET_NAME_LEN: usize = 0xff;
const RADIO_AT: usize = 0x125;
const RADIO_LEN: usize = 0x10;

/// When the codeplug was last written by the vendor software.
///
/// Held as BCD, so a value that is not valid BCD is reported rather than
/// silently turned into a wrong date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    /// Four digit year
    pub year: u16,
    /// 1 to 12
    pub month: u8,
    /// 1 to 31
    pub day: u8,
    /// 0 to 23
    pub hour: u8,
    /// 0 to 59
    pub minute: u8,
    /// 0 to 59
    pub second: u8,
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// The general settings block
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneralSettings {
    /// First line of the startup screen
    pub intro_line1: String,
    /// Second line of the startup screen
    pub intro_line2: String,
    /// The radio's DMR ID
    pub radio_id: u32,
    /// The name the radio calls itself
    pub radio_name: String,
    /// Transmit preamble duration
    pub tx_preamble: u8,
    /// Group call hang time
    pub group_call_hang: u8,
    /// Private call hang time
    pub private_call_hang: u8,
    /// VOX sensitivity
    pub vox_level: u8,
    /// Keypad lock timer
    pub keypad_lock_time: u8,
    /// Operating mode
    pub mode: u8,
}

/// A parsed RDT codeplug
#[derive(Debug, Clone)]
pub struct RdtCodeplug {
    radio: String,
    target_name: String,
    channel_offset: u32,
    timestamp: Option<Timestamp>,
    cps_version: String,
    general: GeneralSettings,
}

impl RdtCodeplug {
    /// The radio model named in the header
    pub fn radio(&self) -> &str {
        &self.radio
    }

    /// The target description in the header
    pub fn target_name(&self) -> &str {
        &self.target_name
    }

    /// Where the channel block starts, as the header records it
    pub fn channel_offset(&self) -> u32 {
        self.channel_offset
    }

    /// When the vendor software last wrote this codeplug, when the stored
    /// value is a valid date
    pub fn timestamp(&self) -> Option<Timestamp> {
        self.timestamp
    }

    /// The CPS software version that wrote it, which radio_tool ignores
    pub fn cps_version(&self) -> &str {
        &self.cps_version
    }

    /// The general settings block
    pub fn general(&self) -> &GeneralSettings {
        &self.general
    }

    /// Does this look like an RDT codeplug
    pub fn is_supported(input: &[u8]) -> bool {
        input.get(..MAGIC.len()) == Some(MAGIC)
            && input.get(TARGET_AT..TARGET_AT + TARGET.len()) == Some(TARGET)
    }

    /// Read a raw memory image, as it comes off a radio.
    ///
    /// A radio hands over its configuration memory with no RDT header on the
    /// front, so there is no magic to check and no radio name to read: those
    /// live in the header a CPS writes. Everything else is in the same place.
    pub fn parse_image(memory: &[u8], radio: &str) -> Result<Self> {
        Self::from_parts(memory, radio.to_owned(), String::new(), 0)
    }

    /// Read a codeplug
    pub fn parse(input: &[u8]) -> Result<Self> {
        if input.get(..MAGIC.len()) != Some(MAGIC) {
            return Err(Error::Malformed("not an RDT codeplug, wrong magic"));
        }
        if input.get(TARGET_AT..TARGET_AT + TARGET.len()) != Some(TARGET) {
            return Err(Error::Malformed("not an RDT codeplug, wrong target tag"));
        }

        let header = input.get(..HEADER_LEN).ok_or(Error::Truncated {
            what: "header",
            wanted: HEADER_LEN,
            got: input.len(),
        })?;

        let channel_offset = read_u32(header, CHANNEL_OFFSET_AT)?;
        let target_name = read_ascii(header, TARGET_NAME_AT, TARGET_NAME_LEN)?;
        let radio = read_ascii(header, RADIO_AT, RADIO_LEN)?;

        // everything below is addressed within the memory image
        let memory = input.get(HEADER_LEN..).ok_or(Error::Truncated {
            what: "memory image",
            wanted: HEADER_LEN,
            got: input.len(),
        })?;

        Self::from_parts(memory, radio, target_name, channel_offset)
    }

    /// The part that reads the memory image, which is the same whether it
    /// came from a file or straight off a radio
    fn from_parts(
        memory: &[u8],
        radio: String,
        target_name: String,
        channel_offset: u32,
    ) -> Result<Self> {
        let stamp_end = TIMESTAMP_OFFSET + TIMESTAMP_LEN + CPS_VERSION_LEN;
        let stamp = memory
            .get(TIMESTAMP_OFFSET..stamp_end)
            .ok_or(Error::Truncated {
                what: "timestamp",
                wanted: stamp_end,
                got: memory.len(),
            })?;

        let timestamp = parse_timestamp(stamp.get(..TIMESTAMP_LEN).unwrap_or_default());
        let cps_version = parse_cps_version(stamp.get(TIMESTAMP_LEN..).unwrap_or_default());

        let settings = memory
            .get(SETTINGS_OFFSET..SETTINGS_OFFSET + SETTINGS_LEN)
            .ok_or(Error::Truncated {
                what: "general settings",
                wanted: SETTINGS_OFFSET + SETTINGS_LEN,
                got: memory.len(),
            })?;

        Ok(Self {
            radio,
            target_name,
            channel_offset,
            timestamp,
            cps_version,
            general: parse_settings(settings)?,
        })
    }
}

impl fmt::Display for RdtCodeplug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, " == RDT Codeplug ==")?;
        writeln!(f, "Radio:   {}", self.radio)?;
        match self.timestamp {
            Some(ts) => writeln!(f, "Created: {ts}")?,
            None => writeln!(f, "Created: Invalid")?,
        }
        if !self.cps_version.is_empty() {
            writeln!(f, "CPS:     {}", self.cps_version)?;
        }
        writeln!(f, "Target:  {}", self.target_name)?;
        writeln!(f, "Radio name: {}", self.general.radio_name)?;
        writeln!(f, "Radio ID:   {}", self.general.radio_id)?;
        write!(
            f,
            "Intro:      {} / {}",
            self.general.intro_line1, self.general.intro_line2
        )
    }
}

/// Seven BCD bytes: century, year, month, day, hour, minute, second
fn parse_timestamp(buf: &[u8]) -> Option<Timestamp> {
    let d: Vec<u8> = buf.iter().map(|b| bcd(*b)).collect();
    let century = *d.first()?;
    let year_in_century = *d.get(1)?;
    let month = *d.get(2)?;
    let day = *d.get(3)?;
    let hour = *d.get(4)?;
    let minute = *d.get(5)?;
    let second = *d.get(6)?;

    // an unwritten or corrupt block must not turn into a plausible date
    if buf.iter().any(|b| (b >> 4) > 9 || (b & 0x0f) > 9) {
        return None;
    }
    let year = u16::from(century) * 100 + u16::from(year_in_century);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    Some(Timestamp {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

/// Four nibbles indexing "0123456789:;<=>?", rendered as Vxx.xx
fn parse_cps_version(buf: &[u8]) -> String {
    const CHARMAP: &[u8; 16] = b"0123456789:;<=>?";
    if buf.len() < CPS_VERSION_LEN {
        return String::new();
    }
    // all zeroes means the field was never written, which is not version 00.00
    if buf.iter().all(|b| *b == 0) {
        return String::new();
    }
    let ch = |ix: usize| {
        buf.get(ix)
            .and_then(|b| CHARMAP.get(usize::from(b & 0x0f)))
            .map(|c| char::from(*c))
            .unwrap_or('?')
    };
    format!("V{}{}.{}{}", ch(0), ch(1), ch(2), ch(3))
}

fn parse_settings(buf: &[u8]) -> Result<GeneralSettings> {
    // offsets within the block, confirmed against dmrconfig's
    // general_settings_t
    Ok(GeneralSettings {
        intro_line1: read_utf16(buf, 0, 10)?,
        intro_line2: read_utf16(buf, 20, 10)?,
        radio_id: {
            let id = buf.get(68..71).ok_or(Error::Malformed("radio id"))?;
            u32::from(*id.first().ok_or(Error::Malformed("radio id"))?)
                | (u32::from(*id.get(1).ok_or(Error::Malformed("radio id"))?) << 8)
                | (u32::from(*id.get(2).ok_or(Error::Malformed("radio id"))?) << 16)
        },
        tx_preamble: *buf.get(72).ok_or(Error::Malformed("settings"))?,
        group_call_hang: *buf.get(73).ok_or(Error::Malformed("settings"))?,
        private_call_hang: *buf.get(74).ok_or(Error::Malformed("settings"))?,
        vox_level: *buf.get(75).ok_or(Error::Malformed("settings"))?,
        keypad_lock_time: *buf.get(86).ok_or(Error::Malformed("settings"))?,
        mode: *buf.get(87).ok_or(Error::Malformed("settings"))?,
        radio_name: read_utf16(buf, 112, 16)?,
    })
}

fn bcd(value: u8) -> u8 {
    ((value >> 4) * 10) + (value & 0x0f)
}

fn read_u32(buf: &[u8], offset: usize) -> Result<u32> {
    let b = buf
        .get(offset..offset + 4)
        .ok_or(Error::Malformed("32 bit field"))?;
    Ok(u32::from_le_bytes([
        *b.first().ok_or(Error::Malformed("32 bit field"))?,
        *b.get(1).ok_or(Error::Malformed("32 bit field"))?,
        *b.get(2).ok_or(Error::Malformed("32 bit field"))?,
        *b.get(3).ok_or(Error::Malformed("32 bit field"))?,
    ]))
}

/// A fixed width ASCII field, truncated at the first null
fn read_ascii(buf: &[u8], offset: usize, len: usize) -> Result<String> {
    let field = buf
        .get(offset..offset + len)
        .ok_or(Error::Malformed("string field"))?;
    let end = field.iter().position(|c| *c == 0).unwrap_or(field.len());
    let text = field.get(..end).ok_or(Error::Malformed("string field"))?;
    Ok(String::from_utf8_lossy(text).trim_end().to_owned())
}

/// A fixed width UTF-16 field, truncated at the first null or 0xffff
fn read_utf16(buf: &[u8], offset: usize, chars: usize) -> Result<String> {
    let field = buf
        .get(offset..offset + (chars * 2))
        .ok_or(Error::Malformed("wide string field"))?;

    let mut units = Vec::with_capacity(chars);
    for pair in field.chunks_exact(2) {
        let Ok(bytes) = <[u8; 2]>::try_from(pair) else {
            break;
        };
        let unit = u16::from_le_bytes(bytes);
        if unit == 0 || unit == 0xffff {
            break;
        }
        units.push(unit);
    }

    Ok(String::from_utf16_lossy(&units).trim_end().to_owned())
}

impl crate::Codeplug for RdtCodeplug {
    fn format(&self) -> &'static str {
        "RDT"
    }

    fn radio(&self) -> String {
        self.radio().to_owned()
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

    /// A codeplug with the fields this module reads filled in
    fn make(radio: &str, timestamp: [u8; 7], version: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0u8; HEADER_LEN + SETTINGS_OFFSET + SETTINGS_LEN + 0x100];

        data[..MAGIC.len()].copy_from_slice(MAGIC);
        data[TARGET_AT..TARGET_AT + TARGET.len()].copy_from_slice(TARGET);
        data[CHANNEL_OFFSET_AT..CHANNEL_OFFSET_AT + 4].copy_from_slice(&0x1234u32.to_le_bytes());

        let name = b"MD-1701 codeplug";
        data[TARGET_NAME_AT..TARGET_NAME_AT + name.len()].copy_from_slice(name);
        data[RADIO_AT..RADIO_AT + radio.len()].copy_from_slice(radio.as_bytes());

        let ts_at = HEADER_LEN + TIMESTAMP_OFFSET;
        data[ts_at..ts_at + 7].copy_from_slice(&timestamp);
        data[ts_at + 7..ts_at + 11].copy_from_slice(&version);

        let gs = HEADER_LEN + SETTINGS_OFFSET;
        let put_utf16 = |d: &mut Vec<u8>, at: usize, text: &str| {
            for (ix, unit) in text.encode_utf16().enumerate() {
                d[at + (ix * 2)..at + (ix * 2) + 2].copy_from_slice(&unit.to_le_bytes());
            }
        };
        put_utf16(&mut data, gs, "HELLO");
        put_utf16(&mut data, gs + 20, "WORLD");
        put_utf16(&mut data, gs + 112, "MYRADIO");
        data[gs + 68..gs + 71].copy_from_slice(&[0x78, 0x56, 0x34]); // 0x345678
        data[gs + 72] = 6; // tx preamble
        data[gs + 87] = 1; // mode

        data
    }

    fn sample() -> Vec<u8> {
        // 2021-10-26 08:56:55, CPS V16.06
        make(
            "DM-1701",
            [0x20, 0x21, 0x10, 0x26, 0x08, 0x56, 0x55],
            [1, 6, 0, 6],
        )
    }

    #[test]
    fn a_codeplug_reads_back() {
        let data = sample();
        assert!(RdtCodeplug::is_supported(&data));

        let cp = RdtCodeplug::parse(&data).expect("parses");
        assert_eq!(cp.radio(), "DM-1701");
        assert_eq!(cp.target_name(), "MD-1701 codeplug");
        assert_eq!(cp.channel_offset(), 0x1234);
    }

    #[test]
    fn the_timestamp_is_bcd() {
        let cp = RdtCodeplug::parse(&sample()).expect("parses");
        let ts = cp.timestamp().expect("a valid date");

        assert_eq!(ts.year, 2021);
        assert_eq!(ts.month, 10);
        assert_eq!(ts.day, 26);
        assert_eq!(ts.hour, 8);
        assert_eq!(ts.minute, 56);
        assert_eq!(ts.second, 55);
        assert_eq!(ts.to_string(), "2021-10-26 08:56:55");
    }

    #[test]
    fn the_cps_version_is_read() {
        // dmrconfig decodes these four nibbles, radio_tool ignores them
        let cp = RdtCodeplug::parse(&sample()).expect("parses");
        assert_eq!(cp.cps_version(), "V16.06");
    }

    #[test]
    fn an_unwritten_cps_version_is_not_reported_as_zero() {
        let data = make(
            "DM-1701",
            [0x20, 0x21, 0x10, 0x26, 0x08, 0x56, 0x55],
            [0; 4],
        );
        let cp = RdtCodeplug::parse(&data).expect("parses");

        assert_eq!(cp.cps_version(), "", "all zeroes is unset, not V00.00");
        assert!(!cp.to_string().contains("CPS"));
    }

    #[test]
    fn an_impossible_date_is_reported_rather_than_guessed() {
        // an unwritten block is all 0xff, which is not valid BCD
        let cp = RdtCodeplug::parse(&make("DM-1701", [0xff; 7], [0; 4])).expect("parses");
        assert_eq!(cp.timestamp(), None);
        assert!(cp.to_string().contains("Invalid"));

        // valid BCD, impossible month
        let cp = RdtCodeplug::parse(&make(
            "DM-1701",
            [0x20, 0x21, 0x13, 0x26, 0x08, 0x56, 0x55],
            [0; 4],
        ))
        .expect("parses");
        assert_eq!(cp.timestamp(), None, "month 13 is not a month");

        // valid BCD, impossible hour
        let cp = RdtCodeplug::parse(&make(
            "DM-1701",
            [0x20, 0x21, 0x10, 0x26, 0x25, 0x56, 0x55],
            [0; 4],
        ))
        .expect("parses");
        assert_eq!(cp.timestamp(), None, "hour 25 is not an hour");
    }

    #[test]
    fn the_general_settings_are_read() {
        let cp = RdtCodeplug::parse(&sample()).expect("parses");
        let g = cp.general();

        assert_eq!(g.intro_line1, "HELLO");
        assert_eq!(g.intro_line2, "WORLD");
        assert_eq!(g.radio_name, "MYRADIO");
        assert_eq!(g.radio_id, 0x345678, "three bytes, little endian");
        assert_eq!(g.tx_preamble, 6);
        assert_eq!(g.mode, 1);
    }

    #[test]
    fn a_wrong_magic_or_target_is_rejected() {
        let mut data = sample();
        data[0] = b'X';
        assert!(!RdtCodeplug::is_supported(&data));
        assert!(RdtCodeplug::parse(&data).is_err());

        let mut data = sample();
        data[TARGET_AT] = b'X';
        assert!(!RdtCodeplug::is_supported(&data));
        assert!(RdtCodeplug::parse(&data).is_err());
    }

    #[test]
    fn truncation_is_reported_not_guessed() {
        let good = sample();

        // everything this module reads ends here, so a file this long is
        // complete as far as we are concerned and anything shorter is not
        let minimum = HEADER_LEN + SETTINGS_OFFSET + SETTINGS_LEN;

        for len in 0..minimum {
            assert!(
                RdtCodeplug::parse(&good[..len]).is_err(),
                "a {len} byte codeplug should not parse, the settings block needs {minimum}"
            );
        }

        assert!(
            RdtCodeplug::parse(&good[..minimum]).is_ok(),
            "{minimum} bytes holds everything we read"
        );
        assert!(good.len() > minimum, "the sample has padding past that");
    }

    #[test]
    fn arbitrary_content_never_panics() {
        let mut x: u32 = 999;
        let mut data: Vec<u8> = (0..0x2200)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x & 0xff) as u8
            })
            .collect();
        data[..MAGIC.len()].copy_from_slice(MAGIC);
        data[TARGET_AT..TARGET_AT + TARGET.len()].copy_from_slice(TARGET);

        if let Ok(cp) = RdtCodeplug::parse(&data) {
            let _ = cp.to_string();
        }
    }
}
