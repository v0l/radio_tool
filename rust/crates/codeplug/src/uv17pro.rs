//! The Baofeng UV-17Pro family codeplug, which includes the UV-5R Mini.
//!
//! A different radio and a different format from the classic UV-5R in
//! [`crate::uv5r`], despite the overlapping names. The image is the radio's
//! memory regions laid end to end, with the CHIRP model name appended, and
//! channels are 32 byte records from offset zero:
//!
//! ```text
//! 0x00  4  receive frequency, little endian BCD, tens of Hz
//! 0x04  4  transmit frequency
//! 0x08  2  receive tone
//! 0x0a  2  transmit tone
//! 0x0c  1  signalling code
//! 0x0d  1  PTT ID
//! 0x0e  1  bits: scramble, and power in the low two
//! 0x0f  1  bits: bandwidth, squelch mode, busy lockout, scan, FHSS
//! 0x10  4  unknown
//! 0x14 12  name
//! ```
//!
//! Layout and bit positions from CHIRP's `baofeng_uv17Pro.py`, which writes
//! its bitfields most significant first. Tones are encoded exactly as the
//! classic UV-5R does, so [`crate::uv5r::Tone`] is reused rather than the
//! table being repeated.
//!
//! One trap in that driver is worth spelling out, because reading the struct
//! alone gets it backwards. The bandwidth bit is *named* `wide`, but it is
//! used inverted:
//!
//! ```python
//! mem.mode = _mem.wide and self.MODES[0] or self.MODES[1]  # ["NFM", "FM"]
//! ```
//!
//! The bit being set means narrow. A radio full of correctly programmed
//! PMR446 channels is what caught this: they are required to be narrow, and
//! reading the bit at face value reported every one of them as wide.
//!
//! A channel in the airband is AM whatever the bit says, which is also
//! decided in the driver rather than stored.

use crate::uv5r::{Power, Tone, strip_chirp_metadata};
use crate::{Error, Result};
use std::fmt;

/// Bytes per channel record
const CHANNEL_LEN: usize = 0x20;
/// Characters in a channel name
const NAME_LEN: usize = 12;
/// Where the name sits inside a record
const NAME_AT: usize = 0x14;

/// A radio in this family
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// Name used on the command line
    pub name: &'static str,
    /// Model string CHIRP stamps after the memory
    pub stamp: &'static str,
    /// Size of the radio's memory, which is where the stamp begins
    pub memory_size: usize,
    /// How many channel records the image holds
    pub channels: usize,
}

/// Every radio this module reads. Sizes and channel counts from CHIRP.
pub const ALL: &[Model] = &[
    Model {
        name: "UV5RMINI",
        stamp: "UV-5R Mini",
        memory_size: 0x8240,
        channels: 999,
    },
    Model {
        name: "UV5GMINI",
        stamp: "UV-5G Mini",
        memory_size: 0x8240,
        channels: 999,
    },
    Model {
        name: "UV17PRO",
        stamp: "UV-17Pro",
        memory_size: 0x8380,
        channels: 1000,
    },
    Model {
        name: "UV17PROGPS",
        stamp: "UV-17ProGPS",
        memory_size: 0x8380,
        channels: 1000,
    },
];

/// Look up a radio by the name used on the command line
pub fn model(name: &str) -> Option<&'static Model> {
    ALL.iter().find(|m| m.name == name)
}

/// Bandwidth and modulation of a channel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 12.5 kHz
    Narrow,
    /// 25 kHz
    Wide,
    /// Airband, which is AM regardless of the bandwidth bit
    Am,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Narrow => "Narrow",
            Self::Wide => "Wide",
            Self::Am => "AM",
        })
    }
}

/// The airband, where a channel is AM whatever its bandwidth bit says
const AIRBAND: std::ops::RangeInclusive<u32> = 108_000_000..=135_999_999;

/// One memory channel
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    /// Receive frequency in Hz
    pub rx_freq: u32,
    /// Transmit frequency in Hz
    pub tx_freq: u32,
    /// Channel name, up to twelve characters
    pub name: String,
    /// Tone squelch on receive
    pub rx_tone: Tone,
    /// Tone sent while transmitting
    pub tx_tone: Tone,
    /// Channel bandwidth and modulation
    pub mode: Mode,
    /// Included in scans
    pub scan: bool,
    /// Busy channel lockout
    pub bcl: bool,
    /// Frequency hopping
    pub fhss: bool,
    /// Transmit power
    pub power: Power,
    /// Signalling code index
    pub scode: u8,
    /// PTT ID mode
    pub pttid: u8,
}

/// A parsed codeplug image
#[derive(Debug, Clone)]
pub struct Uv17ProCodeplug {
    model: &'static Model,
    channels: Vec<Option<Channel>>,
}

impl Uv17ProCodeplug {
    /// The radio this image came from
    pub fn model(&self) -> &'static Model {
        self.model
    }

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

    /// Work out which radio an image is from.
    ///
    /// The stamp after the memory is what CHIRP matches on, and it is the
    /// only thing that tells the two Mini variants apart, since they are the
    /// same size. An image saved by CHIRP keeps the model in its metadata
    /// instead and has no stamp, so size is the fallback.
    pub fn identify(input: &[u8]) -> Option<&'static Model> {
        let image = strip_chirp_metadata(input);

        for m in ALL {
            let Some(tail) = image.get(m.memory_size..) else {
                continue;
            };
            if tail == m.stamp.as_bytes() {
                return Some(m);
            }
        }
        ALL.iter().find(|m| image.len() == m.memory_size)
    }

    /// Does this look like an image from this family
    pub fn is_supported(input: &[u8]) -> bool {
        Self::identify(input).is_some()
    }

    /// Read an image
    pub fn parse(input: &[u8]) -> Result<Self> {
        let image = strip_chirp_metadata(input);
        let model = Self::identify(input).ok_or(Error::Malformed(
            "not a UV-17Pro family image, no model stamp and an unfamiliar size",
        ))?;

        let needed = model.channels * CHANNEL_LEN;
        if image.len() < needed {
            return Err(Error::Truncated {
                what: "channel records",
                wanted: needed,
                got: image.len(),
            });
        }

        let mut channels = Vec::with_capacity(model.channels);
        for ix in 0..model.channels {
            let at = ix * CHANNEL_LEN;
            let record = image
                .get(at..at + CHANNEL_LEN)
                .ok_or(Error::Malformed("channel record"))?;
            channels.push(parse_channel(record)?);
        }

        Ok(Self { model, channels })
    }
}

impl fmt::Display for Uv17ProCodeplug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, " == Baofeng {} Codeplug ==", self.model.stamp)?;
        writeln!(f)?;
        writeln!(
            f,
            "  # Name         RX Freq    TX Freq    RX Tone  TX Tone  BW      Pwr   Scan"
        )?;

        let mut used = 0;
        for (ix, ch) in self.used_channels() {
            used += 1;
            writeln!(
                f,
                "{:3} {:<12} {:>10} {:>10} {:>8} {:>8} {:<7} {:<5} {}",
                ix,
                ch.name,
                format_freq(ch.rx_freq),
                format_freq(ch.tx_freq),
                ch.rx_tone.to_string(),
                ch.tx_tone.to_string(),
                ch.mode.to_string(),
                ch.power.to_string(),
                if ch.scan { "Yes" } else { "No" }
            )?;
        }

        writeln!(f)?;
        write!(f, "{} of {} channels used", used, self.channels.len())
    }
}

/// Read one channel record, returning None for an empty slot
fn parse_channel(record: &[u8]) -> Result<Option<Channel>> {
    let rx_raw = record.get(..4).ok_or(Error::Malformed("channel"))?;

    // an unprogrammed slot is all ones, and a zeroed one is not a channel
    if rx_raw.iter().all(|b| *b == 0xff) || rx_raw.iter().all(|b| *b == 0x00) {
        return Ok(None);
    }

    let rx_freq = read_bcd_freq(record, 0)?;
    let tx_freq = read_bcd_freq(record, 4)?;
    let rx_tone = Tone::from_raw(read_u16(record, 8)?);
    let tx_tone = Tone::from_raw(read_u16(record, 10)?);

    let scode = *record.get(0x0c).ok_or(Error::Malformed("channel"))?;
    let pttid = *record.get(0x0d).ok_or(Error::Malformed("channel"))?;

    // CHIRP writes these most significant first:
    //   0x0e: unknown:2, scramble:2, unknown:2, lowpower:2
    //   0x0f: unknown:1, wide:1, sqmode:2, bcl:1, scan:1, unknown:1, fhss:1
    let power_bits = record.get(0x0e).ok_or(Error::Malformed("channel"))? & 0x03;
    let flags = *record.get(0x0f).ok_or(Error::Malformed("channel"))?;

    Ok(Some(Channel {
        rx_freq,
        tx_freq,
        name: read_name(record, NAME_AT, NAME_LEN),
        rx_tone,
        tx_tone,
        // the bit named `wide` in CHIRP's struct means narrow when set
        mode: if AIRBAND.contains(&rx_freq) {
            Mode::Am
        } else if flags & 0x40 != 0 {
            Mode::Narrow
        } else {
            Mode::Wide
        },
        scan: flags & 0x04 != 0,
        bcl: flags & 0x08 != 0,
        fhss: flags & 0x01 != 0,
        power: if power_bits == 0 {
            Power::High
        } else {
            Power::Low
        },
        scode,
        pttid,
    }))
}

/// Little endian BCD frequency, in units of 10 Hz
fn read_bcd_freq(buf: &[u8], offset: usize) -> Result<u32> {
    let field = buf
        .get(offset..offset + 4)
        .ok_or(Error::Malformed("frequency"))?;

    let mut value: u32 = 0;
    for byte in field.iter().rev() {
        let hi = u32::from(byte >> 4);
        let lo = u32::from(byte & 0x0f);
        if hi > 9 || lo > 9 {
            return Ok(0);
        }
        value = value * 100 + (hi * 10) + lo;
    }
    Ok(value.saturating_mul(10))
}

fn read_u16(buf: &[u8], offset: usize) -> Result<u16> {
    let b = buf
        .get(offset..offset + 2)
        .ok_or(Error::Malformed("16 bit field"))?;
    Ok(u16::from_le_bytes([
        *b.first().ok_or(Error::Malformed("16 bit field"))?,
        *b.get(1).ok_or(Error::Malformed("16 bit field"))?,
    ]))
}

/// A name field, which the radio pads with 0xff and sometimes with nulls
fn read_name(buf: &[u8], offset: usize, len: usize) -> String {
    let Some(field) = buf.get(offset..offset + len) else {
        return String::new();
    };
    let mut out = String::with_capacity(len);
    for byte in field {
        match byte {
            0x00 | 0xff => break,
            c if c.is_ascii_graphic() || *c == b' ' => out.push(char::from(*c)),
            _ => out.push('.'),
        }
    }
    out.trim_end().to_owned()
}

fn format_freq(hz: u32) -> String {
    format!("{}.{:05}", hz / 1_000_000, (hz % 1_000_000) / 10)
}

impl crate::Codeplug for Uv17ProCodeplug {
    fn format(&self) -> &'static str {
        "UV-17Pro"
    }

    fn radio(&self) -> String {
        self.model.stamp.to_owned()
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

    /// An image with a few channels, stamped as a given model
    fn image(m: &Model, channels: &[(u32, &str, u16)]) -> Vec<u8> {
        let mut data = vec![0xffu8; m.memory_size];
        for (ix, (freq, name, tone)) in channels.iter().enumerate() {
            let at = ix * CHANNEL_LEN;
            let bcd = |hz: u32| {
                let mut v = hz / 10;
                let mut out = [0u8; 4];
                for slot in out.iter_mut() {
                    *slot = (((v / 10) % 10) << 4) as u8 | (v % 10) as u8;
                    v /= 100;
                }
                out
            };
            data[at..at + 4].copy_from_slice(&bcd(*freq));
            data[at + 4..at + 8].copy_from_slice(&bcd(*freq));
            data[at + 8..at + 10].copy_from_slice(&0u16.to_le_bytes());
            data[at + 10..at + 12].copy_from_slice(&tone.to_le_bytes());
            data[at + 0x0e] = 0x00;
            data[at + 0x0f] = 0x44; // bandwidth bit set, meaning narrow, and scan
            let name = name.as_bytes();
            data[at + NAME_AT..at + NAME_AT + name.len()].copy_from_slice(name);
        }
        data.extend_from_slice(m.stamp.as_bytes());
        data
    }

    #[test]
    fn every_model_is_consistent() {
        for m in ALL {
            assert!(m.channels * CHANNEL_LEN <= m.memory_size, "{}", m.name);
            assert!(!m.stamp.is_empty(), "{}", m.name);
        }
        assert!(model("UV5RMINI").is_some());
        assert!(model("NOPE").is_none());
    }

    #[test]
    fn the_model_stamp_identifies_the_radio() {
        for m in ALL {
            let data = image(m, &[]);
            assert_eq!(
                Uv17ProCodeplug::identify(&data).map(|f| f.name),
                Some(m.name),
                "{} was not identified by its stamp",
                m.name
            );
        }
    }

    #[test]
    fn an_image_without_a_stamp_falls_back_to_its_size() {
        let m = model("UV5RMINI").expect("known");
        let mut data = image(m, &[]);
        data.truncate(m.memory_size);

        // the two Mini variants are the same size, so this can only narrow it
        // to one of them, and says so by returning the first
        let found = Uv17ProCodeplug::identify(&data).expect("identified by size");
        assert_eq!(found.memory_size, m.memory_size);
    }

    #[test]
    fn channels_read_back() {
        let m = model("UV5RMINI").expect("known");
        let data = image(
            m,
            &[
                (446_006_250, "PMR01", 0),
                (446_018_750, "PMR02", 1413),
                (145_500_000, "SIMPLEX", 0),
            ],
        );

        let cp = Uv17ProCodeplug::parse(&data).expect("parses");
        assert_eq!(cp.model().name, "UV5RMINI");
        assert_eq!(cp.channels().len(), 999);

        let used: Vec<_> = cp.used_channels().collect();
        assert_eq!(used.len(), 3);

        let (ix, first) = used[0];
        assert_eq!(ix, 0);
        assert_eq!(first.name, "PMR01");
        assert_eq!(first.rx_freq, 446_006_250);
        assert_eq!(first.tx_freq, 446_006_250);
        assert_eq!(
            first.mode,
            Mode::Narrow,
            "the bandwidth bit is set, and set means narrow"
        );
        assert!(first.scan);
        assert_eq!(first.power, Power::High);

        let (_, second) = used[1];
        assert_eq!(second.tx_tone, Tone::Ctcss { tenths: 1413 });
        assert_eq!(second.tx_tone.to_string(), "141.3");
        assert_eq!(second.rx_tone, Tone::None);
    }

    /// CHIRP names this bit `wide` and then uses it inverted, so reading the
    /// struct at face value gets every channel's bandwidth backwards
    #[test]
    fn the_bandwidth_bit_set_means_narrow() {
        let m = model("UV5RMINI").expect("known");
        let mut data = image(m, &[(446_006_250, "PMR01", 0)]);

        assert_eq!(
            Uv17ProCodeplug::parse(&data).expect("parses").channels()[0]
                .as_ref()
                .expect("channel")
                .mode,
            Mode::Narrow
        );

        data[0x0f] &= !0x40;
        assert_eq!(
            Uv17ProCodeplug::parse(&data).expect("parses").channels()[0]
                .as_ref()
                .expect("channel")
                .mode,
            Mode::Wide,
            "clearing the bit is what means wide"
        );
    }

    #[test]
    fn an_airband_channel_is_am_whatever_the_bandwidth_bit_says() {
        let m = model("UV5RMINI").expect("known");
        let mut data = image(m, &[(127_500_000, "TOWER", 0)]);

        for bit in [0x40u8, 0x00] {
            data[0x0f] = (data[0x0f] & !0x40) | bit;
            assert_eq!(
                Uv17ProCodeplug::parse(&data).expect("parses").channels()[0]
                    .as_ref()
                    .expect("channel")
                    .mode,
                Mode::Am
            );
        }
    }

    #[test]
    fn an_empty_image_has_no_channels() {
        let m = model("UV17PRO").expect("known");
        let cp = Uv17ProCodeplug::parse(&image(m, &[])).expect("parses");
        assert_eq!(cp.channels().len(), 1000);
        assert_eq!(cp.used_channels().count(), 0);
    }

    #[test]
    fn a_short_or_unfamiliar_image_is_refused() {
        assert!(!Uv17ProCodeplug::is_supported(&[]));
        assert!(!Uv17ProCodeplug::is_supported(&[0xff; 1024]));
        assert!(Uv17ProCodeplug::parse(&[0xff; 1024]).is_err());

        // the right size but truncated after identification
        let m = model("UV5RMINI").expect("known");
        let mut data = image(m, &[]);
        data.truncate(m.memory_size - 1);
        assert!(Uv17ProCodeplug::parse(&data).is_err());
    }

    #[test]
    fn arbitrary_content_never_panics() {
        let m = model("UV5RMINI").expect("known");
        let mut x: u32 = 1234;
        let mut data: Vec<u8> = (0..m.memory_size)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x & 0xff) as u8
            })
            .collect();
        data.extend_from_slice(m.stamp.as_bytes());

        if let Ok(cp) = Uv17ProCodeplug::parse(&data) {
            let _ = cp.to_string();
        }

        for len in (0..0x400).step_by(7) {
            let _ = Uv17ProCodeplug::parse(&data[..len.min(data.len())]);
        }
    }
}
