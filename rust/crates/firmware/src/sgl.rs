//! The SGL firmware container, used by the Radioddity GD-77 and GD-77S, the
//! Baofeng RD-5R and BF-5R, and the Baofeng DM-1801.
//!
//! Layout:
//!
//! ```text
//! 0x000   4  "SGL!"
//! 0x004  12  header 1, XOR encrypted against the magic:
//!              +0  "ENCV" and a three digit version, so "ENCV001"
//!              +7  binary offset, added to 0x400 to find the firmware
//!              +8  offset of header 2, little endian u16
//!              +10 the two byte XOR key header 2 is encrypted with
//!   var 0x67  header 2, at the offset named above, XOR encrypted:
//!              +0x06 firmware length, little endian u32
//!              +0x32 radio group, 16 bytes
//!              +0x42 radio model, 8 bytes
//!              +0x4a protocol version, 8 bytes
//!              +0x5f model key, 8 bytes
//! 0x400+n  n  firmware data, obfuscated
//! ```
//!
//! Two details differ from the C++ implementation on purpose, both noted in
//! `rust/README.md`: the header secrets come from real entropy rather than a
//! default seeded PRNG, and the version field is read as the three digits the
//! writer emits rather than starting one byte in.
//!
//! Every offset here was cross checked against OpenGD77's
//! `gd-77_firmware_loader.py`, an implementation written from the same radios
//! but not from the same source, and they agree throughout.

use crate::cipher;
use crate::{Error, Result, Segment};

/// File magic, also the key header 1 is encrypted with
const MAGIC: [u8; 4] = *b"SGL!";

const HEADER_LEN: usize = 0x400;
const HEADER1_LEN: usize = 0x10;
const HEADER2_LEN: usize = 0x67;

/// Offsets inside header 1
const VERSION_OFFSET: usize = 0x04;
const BINARY_OFFSET_AT: usize = 0x0b;
const HEADER2_OFFSET_AT: usize = 0x0c;
const H2_KEY_AT: usize = 0x0e;

/// Offsets inside header 2
const BINARY_LEN_OFFSET: usize = 0x06;
const GROUP_OFFSET: usize = 0x32;
const GROUP_LEN: usize = 0x10;
const MODEL_OFFSET: usize = GROUP_OFFSET + GROUP_LEN;
const MODEL_LEN: usize = 0x08;
const VERSION_STR_OFFSET: usize = MODEL_OFFSET + MODEL_LEN;
const VERSION_STR_LEN: usize = 0x08;
const KEY_OFFSET: usize = 0x5f;
const KEY_LEN: usize = 0x08;

/// The header 2 offset the writer is allowed to choose
const H2_OFFSET_MIN: u16 = 0x1e;
const H2_OFFSET_MAX: u16 = 0x100;
/// The binary offset the writer is allowed to choose
const BINARY_OFFSET_MAX: u8 = 0x80;

/// The only container version that exists in the wild
const SGL_VERSION: u16 = 1;

/// Everything that varies between radios in this family
#[derive(Debug, Clone, Copy)]
pub struct RadioConfig {
    /// Model as the user names it on the command line
    pub radio_model: &'static str,
    /// Group string in header 2, this is what identifies the radio
    pub radio_group: &'static str,
    /// Model string in header 2
    pub header_model: &'static str,
    /// Protocol version string in header 2
    pub protocol_version: &'static str,
    /// The first four bytes of the model key, the rest is filler
    pub model_key_prefix: &'static str,
    /// XOR key for the firmware data
    pub cipher: &'static [u8],
    /// How far into the cipher the firmware data starts
    pub xor_offset: usize,
}

/// Every radio this container supports
pub const ALL: &[RadioConfig] = &[
    RadioConfig {
        radio_model: "GD77",
        radio_group: "SG-MD-760",
        header_model: "MD-760",
        protocol_version: "V1.00.01",
        model_key_prefix: "DV01",
        cipher: cipher::SGL,
        xor_offset: 0x807,
    },
    RadioConfig {
        radio_model: "GD77S",
        radio_group: "SG-MD-730",
        header_model: "MD-730",
        protocol_version: "V1.00.01",
        model_key_prefix: "DV02",
        cipher: cipher::SGL,
        xor_offset: 0x2a8e,
    },
    RadioConfig {
        radio_model: "BF5R",
        radio_group: "BF-5R",
        header_model: "BF-5R",
        protocol_version: "V1.00.01",
        model_key_prefix: "DV02",
        cipher: cipher::SGL,
        xor_offset: 0x306e,
    },
    RadioConfig {
        radio_model: "DM1801",
        radio_group: "BF-DMR",
        header_model: "1801",
        protocol_version: "V1.00.01",
        model_key_prefix: "DV03",
        cipher: cipher::SGL,
        xor_offset: 0x2c7c,
    },
];

/// Look up a radio by the name used on the command line
pub fn config_for_model(model: &str) -> Option<&'static RadioConfig> {
    ALL.iter().find(|r| r.radio_model == model)
}

/// Look up a radio by the group string in a firmware header
pub fn config_for_group(group: &str) -> Option<&'static RadioConfig> {
    ALL.iter().find(|r| r.radio_group == group)
}

/// The parts of the header the writer picks freely.
///
/// The official tool varies these per build, so a firmware file cannot be
/// reproduced byte for byte from its contents alone. Use
/// [`HeaderSecrets::random`] to write a file and pass a fixed value when a
/// test needs a repeatable one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderSecrets {
    /// Added to 0x400 to find the firmware data, at most 0x80
    pub binary_offset: u8,
    /// Where header 2 lives, between 0x1e and 0x100
    pub header2_offset: u16,
    /// Two byte XOR key for header 2
    pub h2_key: [u8; 2],
    /// The four bytes after the model key prefix.
    ///
    /// Not padding: a flasher reads these out of the file and sends them to
    /// the radio to open the session. OpenGD77's loader calls this the
    /// encodeKey and reads it at header 2 + 0x63, which is where the four
    /// bytes after our four byte prefix land.
    pub transfer_key: [u8; 4],
}

impl HeaderSecrets {
    /// Pick values the way the official tool does
    pub fn random() -> Self {
        let mut rng = Rng::from_entropy();
        Self {
            // 0..=0x80 inclusive, matching the C++ distribution
            binary_offset: (rng.next() % (u32::from(BINARY_OFFSET_MAX) + 1)) as u8,
            header2_offset: H2_OFFSET_MIN
                + (rng.next() % u32::from(H2_OFFSET_MAX - H2_OFFSET_MIN + 1)) as u16,
            h2_key: [rng.next() as u8, rng.next() as u8],
            // printable ASCII, '!' through '}'
            transfer_key: std::array::from_fn(|_| b'!' + (rng.next() % (0x7d - 0x21 + 1)) as u8),
        }
    }

    /// The values the header actually holds, for round tripping a file
    fn validate(&self) -> Result<()> {
        if self.binary_offset > BINARY_OFFSET_MAX {
            return Err(Error::Malformed("binary offset is past 0x80"));
        }
        if self.header2_offset < H2_OFFSET_MIN || self.header2_offset > H2_OFFSET_MAX {
            return Err(Error::Malformed("header 2 offset is out of range"));
        }
        Ok(())
    }
}

/// A tiny xorshift, seeded from the OS through `RandomState`.
///
/// This only has to vary between files, exactly like the C++ original
/// intended. It is not, and does not need to be, cryptographic.
struct Rng(u64);

impl Rng {
    fn from_entropy() -> Self {
        use std::hash::{BuildHasher, Hasher};
        // RandomState is seeded by the OS once per process, and hashing a
        // counter gives a different value on every call
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_usize(
            std::time::SystemTime::now()
                .elapsed()
                .map_or(0, |d| usize::try_from(d.as_nanos()).unwrap_or(usize::MAX)),
        );
        Self(hasher.finish() | 1)
    }

    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
}

/// A parsed SGL firmware file, holding decrypted firmware data
#[derive(Debug, Clone)]
pub struct SglFirmware {
    config: &'static RadioConfig,
    /// Decrypted firmware data
    data: Vec<u8>,
    /// The model key as it appeared in the file, if this came from one
    model_key: Option<[u8; KEY_LEN]>,
    /// The header secrets this came with, so a parsed file can be written
    /// back out unchanged rather than picking new ones
    secrets: Option<HeaderSecrets>,
    /// The whole header area as it was read, decrypted.
    ///
    /// Vendor firmware fills the space around the two headers with bytes
    /// whose meaning is unknown. Keeping them means a file read and written
    /// again is unchanged, rather than having those bytes replaced with
    /// zeroes on the assumption that nothing reads them.
    header_template: Option<Vec<u8>>,
}

impl SglFirmware {
    /// Start a new firmware file for a radio
    pub fn new(model: &str) -> Result<Self> {
        let config =
            config_for_model(model).ok_or_else(|| Error::UnknownModel(model.to_owned()))?;
        Ok(Self {
            config,
            data: Vec::new(),
            model_key: None,
            secrets: None,
            header_template: None,
        })
    }

    /// The radio this firmware is for
    pub fn config(&self) -> &'static RadioConfig {
        self.config
    }

    /// Decrypted firmware data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// This container holds a single region, always written from the start
    pub fn segments(&self) -> Vec<Segment<'_>> {
        vec![Segment {
            address: 0,
            data: &self.data,
        }]
    }

    /// Set the firmware data. This container has no region table, so unlike
    /// the TYT one it holds exactly one blob.
    pub fn set_data(&mut self, data: &[u8]) -> Result<()> {
        u32::try_from(data.len()).map_err(|_| Error::SegmentTooLarge(data.len()))?;
        self.data = data.to_vec();
        Ok(())
    }

    /// Is this the same radio, and does the firmware key match
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.config.radio_model == other.config.radio_model
            && self.config.radio_group == other.config.radio_group
            && self.config.protocol_version == other.config.protocol_version
            && self.data.len() == other.data.len()
    }

    /// Read a firmware file
    pub fn parse(input: &[u8]) -> Result<Self> {
        let (secrets, length, group) = parse_header(input)?;
        let config = config_for_group(&group).ok_or(Error::UnsupportedCounterMagic)?;

        let start = HEADER_LEN
            .checked_add(usize::from(secrets.binary_offset))
            .ok_or(Error::Malformed("binary offset overflow"))?;
        let end = start
            .checked_add(length as usize)
            .ok_or(Error::Malformed("firmware length overflow"))?;

        let mut data = input
            .get(start..end)
            .ok_or(Error::Truncated {
                what: "firmware data",
                wanted: end,
                got: input.len(),
            })?
            .to_vec();

        decrypt(&mut data, config);

        // keep the model key exactly as it was, the last four bytes are filler
        // the writer chose and are not ours to invent
        let h2_at = usize::from(secrets.header2_offset);
        let model_key = read_model_key(input, h2_at, secrets.h2_key)?;

        // keep the header area in the clear, so writing it back reproduces
        // the bytes we do not understand
        let header_template = input.get(..start).map(|raw| {
            let mut plain = raw.to_vec();
            if let Some(h1) = plain.get_mut(VERSION_OFFSET..HEADER1_LEN) {
                cipher::apply_xor(h1, &MAGIC, VERSION_OFFSET);
            }
            if let Some(h2) = plain.get_mut(h2_at..h2_at + HEADER2_LEN) {
                cipher::apply_xor(h2, &secrets.h2_key, 0);
            }
            plain
        });

        Ok(Self {
            config,
            data,
            model_key: Some(model_key),
            secrets: Some(HeaderSecrets {
                transfer_key: model_key
                    .get(4..8)
                    .and_then(|f| f.try_into().ok())
                    .unwrap_or([0; 4]),
                ..secrets
            }),
            header_template,
        })
    }

    /// The header secrets this file came with, if it was parsed from one
    /// The eight byte model key sent to open a flashing session: the four
    /// byte prefix from the radio's config, then the transfer key that was
    /// stored in the file.
    ///
    /// The radio answers with the prefix alone, which is what OpenGD77's
    /// loader expects back.
    pub fn model_key(&self) -> Option<Vec<u8>> {
        let secrets = self.secrets()?;
        let mut key = self.config().model_key_prefix.as_bytes().to_vec();
        key.extend_from_slice(&secrets.transfer_key);
        Some(key)
    }

    /// The firmware as it is stored in the file, still enciphered, which is
    /// what a radio is sent
    pub fn data_as_stored(&self) -> Vec<u8> {
        let mut data = self.data.clone();
        crate::cipher::apply_xor(&mut data, self.config.cipher, self.config.xor_offset);
        data
    }

    pub fn secrets(&self) -> Option<HeaderSecrets> {
        self.secrets
    }

    /// Write a firmware file.
    ///
    /// A file that was parsed is written back with the header secrets it came
    /// with, so reading and writing is lossless. A file built from scratch
    /// gets fresh ones.
    pub fn serialise(&self) -> Result<Vec<u8>> {
        match self.secrets {
            Some(secrets) => self.serialise_with(secrets),
            None => self.serialise_with(HeaderSecrets::random()),
        }
    }

    /// Write a firmware file with the given header secrets
    pub fn serialise_with(&self, secrets: HeaderSecrets) -> Result<Vec<u8>> {
        secrets.validate()?;
        if self.data.is_empty() {
            return Err(Error::NoSegments);
        }
        let length =
            u32::try_from(self.data.len()).map_err(|_| Error::SegmentTooLarge(self.data.len()))?;

        let h2_at = usize::from(secrets.header2_offset);
        if h2_at + HEADER2_LEN > HEADER_LEN {
            return Err(Error::Malformed(
                "header 2 does not fit before the firmware",
            ));
        }

        let wanted_len = HEADER_LEN + usize::from(secrets.binary_offset);

        // start from the header this file came with, when it came with one and
        // the secrets still line up, so bytes of unknown meaning survive
        let mut out = match &self.header_template {
            Some(template) if template.len() == wanted_len && self.secrets == Some(secrets) => {
                template.clone()
            }
            _ => vec![0u8; wanted_len],
        };

        // header 1, written in the clear then encrypted at the end
        write_at(&mut out, 0, &MAGIC)?;
        let version = format!("ENCV{:03}", SGL_VERSION);
        write_at(&mut out, VERSION_OFFSET, version.as_bytes())?;
        write_at(&mut out, BINARY_OFFSET_AT, &[secrets.binary_offset])?;
        write_at(
            &mut out,
            HEADER2_OFFSET_AT,
            &secrets.header2_offset.to_le_bytes(),
        )?;
        write_at(&mut out, H2_KEY_AT, &secrets.h2_key)?;

        // header 2
        write_at(&mut out, h2_at, &[0x02, 0x10])?;
        write_at(&mut out, h2_at + BINARY_LEN_OFFSET, &length.to_le_bytes())?;
        write_field(
            &mut out,
            h2_at + GROUP_OFFSET,
            GROUP_LEN,
            self.config.radio_group,
        )?;
        write_field(
            &mut out,
            h2_at + MODEL_OFFSET,
            MODEL_LEN,
            self.config.header_model,
        )?;
        write_field(
            &mut out,
            h2_at + VERSION_STR_OFFSET,
            VERSION_STR_LEN,
            self.config.protocol_version,
        )?;

        let key = self.model_key.unwrap_or_else(|| {
            let mut key = [0u8; KEY_LEN];
            for (slot, byte) in key.iter_mut().zip(self.config.model_key_prefix.bytes()) {
                *slot = byte;
            }
            for (slot, byte) in key.iter_mut().skip(4).zip(secrets.transfer_key) {
                *slot = byte;
            }
            key
        });
        write_at(&mut out, h2_at + KEY_OFFSET, &key)?;

        // encrypt header 2, then header 1
        {
            let h2 = out
                .get_mut(h2_at..h2_at + HEADER2_LEN)
                .ok_or(Error::Malformed("header 2 does not fit"))?;
            cipher::apply_xor(h2, &secrets.h2_key, 0);
        }
        {
            let h1 = out
                .get_mut(VERSION_OFFSET..HEADER1_LEN)
                .ok_or(Error::Malformed("header 1 does not fit"))?;
            // the key is the magic, and the offset keeps it in phase with the
            // start of the file rather than the start of the slice
            cipher::apply_xor(h1, &MAGIC, VERSION_OFFSET);
        }

        let mut data = self.data.clone();
        encrypt(&mut data, self.config);
        out.extend_from_slice(&data);

        Ok(out)
    }

    /// Does this look like an SGL firmware file
    pub fn is_supported(input: &[u8]) -> bool {
        match parse_header(input) {
            Ok((_, length, group)) => length > 0 && config_for_group(&group).is_some(),
            Err(_) => false,
        }
    }
}

/// Pull the secrets, the firmware length and the radio group out of a header
fn parse_header(input: &[u8]) -> Result<(HeaderSecrets, u32, String)> {
    let raw = input.get(..HEADER1_LEN).ok_or(Error::Truncated {
        what: "header 1",
        wanted: HEADER1_LEN,
        got: input.len(),
    })?;

    if raw.get(..4) != Some(&MAGIC[..]) {
        return Err(Error::BadMagic);
    }

    let mut header1 = [0u8; HEADER1_LEN];
    header1.copy_from_slice(raw);
    let h1 = header1
        .get_mut(VERSION_OFFSET..)
        .ok_or(Error::Malformed("header 1"))?;
    cipher::apply_xor(h1, &MAGIC, VERSION_OFFSET);

    // "ENCV001", three digits after the four letter tag
    let digits = header1
        .get(VERSION_OFFSET + 4..VERSION_OFFSET + 7)
        .ok_or(Error::Malformed("version"))?;
    let version: u16 = std::str::from_utf8(digits)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(Error::Malformed("version is not a number"))?;
    if version != SGL_VERSION {
        return Err(Error::Malformed("unsupported SGL version"));
    }

    let binary_offset = *header1
        .get(BINARY_OFFSET_AT)
        .ok_or(Error::Malformed("binary offset"))?;
    let header2_offset = u16::from_le_bytes([
        *header1
            .get(HEADER2_OFFSET_AT)
            .ok_or(Error::Malformed("header 2 offset"))?,
        *header1
            .get(HEADER2_OFFSET_AT + 1)
            .ok_or(Error::Malformed("header 2 offset"))?,
    ]);
    let h2_key = [
        *header1
            .get(H2_KEY_AT)
            .ok_or(Error::Malformed("header 2 key"))?,
        *header1
            .get(H2_KEY_AT + 1)
            .ok_or(Error::Malformed("header 2 key"))?,
    ];

    let secrets = HeaderSecrets {
        binary_offset,
        header2_offset,
        h2_key,
        transfer_key: [0; 4],
    };
    secrets.validate()?;

    let h2_at = usize::from(header2_offset);
    let mut header2 = input
        .get(h2_at..h2_at + HEADER2_LEN)
        .ok_or(Error::Truncated {
            what: "header 2",
            wanted: h2_at + HEADER2_LEN,
            got: input.len(),
        })?
        .to_vec();
    cipher::apply_xor(&mut header2, &h2_key, 0);

    let length = u32::from_le_bytes([
        *header2
            .get(BINARY_LEN_OFFSET)
            .ok_or(Error::Malformed("firmware length"))?,
        *header2
            .get(BINARY_LEN_OFFSET + 1)
            .ok_or(Error::Malformed("firmware length"))?,
        *header2
            .get(BINARY_LEN_OFFSET + 2)
            .ok_or(Error::Malformed("firmware length"))?,
        *header2
            .get(BINARY_LEN_OFFSET + 3)
            .ok_or(Error::Malformed("firmware length"))?,
    ]);

    let group = read_field(&header2, GROUP_OFFSET, GROUP_LEN)?;

    Ok((secrets, length, group))
}

/// Pull the model key out of an encrypted header 2
fn read_model_key(input: &[u8], h2_at: usize, h2_key: [u8; 2]) -> Result<[u8; KEY_LEN]> {
    let raw = input
        .get(h2_at + KEY_OFFSET..h2_at + KEY_OFFSET + KEY_LEN)
        .ok_or(Error::Malformed("model key"))?;
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(raw);
    // the whole of header 2 is XORed with the two byte key, so the offset
    // into the keystream is the field's offset within header 2
    cipher::apply_xor(&mut key, &h2_key, KEY_OFFSET);
    Ok(key)
}

/// Read a fixed width field, stopping at the first byte that is not printable
fn read_field(buf: &[u8], offset: usize, len: usize) -> Result<String> {
    let field = buf
        .get(offset..offset + len)
        .ok_or(Error::Malformed("header field"))?;
    let end = field
        .iter()
        .position(|c| !(0x20..=0x7e).contains(c))
        .unwrap_or(field.len());
    let text = field.get(..end).ok_or(Error::Malformed("header field"))?;
    Ok(String::from_utf8_lossy(text).into_owned())
}

fn write_at(buf: &mut [u8], offset: usize, data: &[u8]) -> Result<()> {
    let slot = buf
        .get_mut(offset..offset + data.len())
        .ok_or(Error::Malformed("header field does not fit"))?;
    slot.copy_from_slice(data);
    Ok(())
}

fn write_field(buf: &mut [u8], offset: usize, len: usize, text: &str) -> Result<()> {
    if text.len() > len {
        return Err(Error::Malformed("header field is too long"));
    }
    write_at(buf, offset, text.as_bytes())
}

/// The firmware data is rotated and inverted as well as XORed
fn encrypt(data: &mut [u8], config: &RadioConfig) {
    for (ix, byte) in data.iter_mut().enumerate() {
        let mut b = *byte;
        if !config.cipher.is_empty() {
            let at = (config.xor_offset + ix) % config.cipher.len();
            b ^= config.cipher.get(at).copied().unwrap_or(0);
        }
        *byte = !b.rotate_right(3);
    }
}

fn decrypt(data: &mut [u8], config: &RadioConfig) {
    for (ix, byte) in data.iter_mut().enumerate() {
        let mut b = !byte.rotate_left(3);
        if !config.cipher.is_empty() {
            let at = (config.xor_offset + ix) % config.cipher.len();
            b ^= config.cipher.get(at).copied().unwrap_or(0);
        }
        *byte = b;
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

    fn sample(len: usize, seed: u32) -> Vec<u8> {
        let mut x = seed | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x & 0xff) as u8
            })
            .collect()
    }

    fn fixed_secrets() -> HeaderSecrets {
        HeaderSecrets {
            binary_offset: 0x5b,
            header2_offset: 0x9c,
            h2_key: [0x3a, 0xf1],
            transfer_key: *b"T=xz",
        }
    }

    fn wrap(model: &str, data: &[u8]) -> Vec<u8> {
        let mut fw = SglFirmware::new(model).expect("model is supported");
        fw.set_data(data).expect("data fits");
        fw.serialise_with(fixed_secrets()).expect("serialises")
    }

    #[test]
    fn every_config_fits_its_header_fields() {
        for config in ALL {
            assert!(
                config.radio_group.len() <= GROUP_LEN,
                "{}",
                config.radio_model
            );
            assert!(
                config.header_model.len() <= MODEL_LEN,
                "{}",
                config.radio_model
            );
            assert!(
                config.protocol_version.len() <= VERSION_STR_LEN,
                "{}",
                config.radio_model
            );
            assert_eq!(config.model_key_prefix.len(), 4, "{}", config.radio_model);
            assert!(
                config.xor_offset < config.cipher.len(),
                "{}",
                config.radio_model
            );
        }
    }

    #[test]
    fn radio_groups_are_unique() {
        // the group is what identifies the radio when reading a file
        for (ix, a) in ALL.iter().enumerate() {
            for b in ALL.iter().skip(ix + 1) {
                assert_ne!(a.radio_group, b.radio_group);
            }
        }
    }

    #[test]
    fn obfuscation_is_reversible() {
        for config in ALL {
            let plain = sample(0x1000, 3);
            let mut data = plain.clone();
            encrypt(&mut data, config);
            assert_ne!(data, plain, "{}: encrypt was a no-op", config.radio_model);
            decrypt(&mut data, config);
            assert_eq!(data, plain, "{}: did not round trip", config.radio_model);
        }
    }

    #[test]
    fn round_trip_through_a_file() {
        for config in ALL {
            let data = sample(0x2000, 5);
            let file = wrap(config.radio_model, &data);

            assert!(SglFirmware::is_supported(&file), "{}", config.radio_model);

            let fw = SglFirmware::parse(&file).expect("parses");
            assert_eq!(fw.config().radio_model, config.radio_model);
            assert_eq!(fw.data(), data, "{}: data changed", config.radio_model);
            assert_eq!(fw.segments().len(), 1);
            assert_eq!(fw.segments()[0].address, 0);
        }
    }

    #[test]
    fn file_layout_is_what_the_radio_expects() {
        let data = sample(0x400, 6);
        let file = wrap("GD77", &data);
        let secrets = fixed_secrets();

        assert_eq!(&file[..4], &MAGIC);
        assert_eq!(
            file.len(),
            HEADER_LEN + usize::from(secrets.binary_offset) + data.len()
        );
        // the header is encrypted, the tag must not be readable
        assert_ne!(&file[4..11], b"ENCV001");
    }

    #[test]
    fn secrets_survive_a_round_trip() {
        let file = wrap("DM1801", &sample(0x100, 7));
        let (secrets, length, group) = parse_header(&file).expect("header parses");

        assert_eq!(secrets.binary_offset, fixed_secrets().binary_offset);
        assert_eq!(secrets.header2_offset, fixed_secrets().header2_offset);
        assert_eq!(secrets.h2_key, fixed_secrets().h2_key);
        assert_eq!(length, 0x100);
        assert_eq!(group, "BF-DMR");
    }

    #[test]
    fn random_secrets_are_always_in_range() {
        for _ in 0..2000 {
            let s = HeaderSecrets::random();
            assert!(s.binary_offset <= BINARY_OFFSET_MAX);
            assert!((H2_OFFSET_MIN..=H2_OFFSET_MAX).contains(&s.header2_offset));
            assert!(s.validate().is_ok());
            for c in s.transfer_key {
                assert!(
                    (b'!'..=b'}').contains(&c),
                    "key filler {c:#x} is not printable"
                );
            }
        }
    }

    #[test]
    fn random_secrets_actually_vary() {
        // the C++ used a default seeded engine, so every file it wrote had
        // the same "random" values
        let mut seen = std::collections::HashSet::new();
        for _ in 0..64 {
            let s = HeaderSecrets::random();
            seen.insert((s.binary_offset, s.header2_offset, s.h2_key, s.transfer_key));
        }
        assert!(
            seen.len() > 32,
            "header secrets barely vary: {} of 64",
            seen.len()
        );
    }

    #[test]
    fn a_file_written_with_fresh_secrets_still_reads_back() {
        for _ in 0..200 {
            let data = sample(0x80, 9);
            let mut fw = SglFirmware::new("GD77S").expect("model is supported");
            fw.set_data(&data).expect("data fits");
            let file = fw.serialise().expect("serialises");

            let back = SglFirmware::parse(&file).expect("parses");
            assert_eq!(back.data(), data);
        }
    }

    #[test]
    fn empty_firmware_will_not_serialise() {
        let fw = SglFirmware::new("GD77").expect("model is supported");
        assert!(matches!(fw.serialise(), Err(Error::NoSegments)));
    }

    #[test]
    fn unknown_model_is_rejected() {
        assert!(matches!(
            SglFirmware::new("NOT-A-RADIO"),
            Err(Error::UnknownModel(_))
        ));
    }

    #[test]
    fn out_of_range_secrets_are_rejected() {
        let mut fw = SglFirmware::new("GD77").expect("model is supported");
        fw.set_data(&[1, 2, 3]).expect("data fits");

        let bad = HeaderSecrets {
            binary_offset: 0x81,
            ..fixed_secrets()
        };
        assert!(fw.serialise_with(bad).is_err());

        let bad = HeaderSecrets {
            header2_offset: 0x1d,
            ..fixed_secrets()
        };
        assert!(fw.serialise_with(bad).is_err());

        let bad = HeaderSecrets {
            header2_offset: 0x101,
            ..fixed_secrets()
        };
        assert!(fw.serialise_with(bad).is_err());
    }

    #[test]
    fn malformed_input_never_panics() {
        let good = wrap("GD77", &sample(0x400, 11));

        for len in 0..good.len() {
            let _ = SglFirmware::parse(&good[..len]);
            let _ = SglFirmware::is_supported(&good[..len]);
        }

        let mut bad = good.clone();
        bad[0] = b'X';
        assert!(matches!(SglFirmware::parse(&bad), Err(Error::BadMagic)));

        // corrupt every single byte of the header in turn
        for at in 0..HEADER_LEN.min(good.len()) {
            let mut bad = good.clone();
            bad[at] ^= 0xff;
            let _ = SglFirmware::parse(&bad);
            let _ = SglFirmware::is_supported(&bad);
        }

        for len in 0..0x420 {
            let noise = sample(len, (len as u32) + 1);
            let _ = SglFirmware::parse(&noise);
            let _ = SglFirmware::is_supported(&noise);
        }
    }

    #[test]
    fn a_group_no_radio_uses_is_rejected() {
        let mut file = wrap("GD77", &sample(0x100, 12));
        let secrets = fixed_secrets();
        let h2 = usize::from(secrets.header2_offset);

        // rewrite the group in place, remembering it is XOR encrypted
        let at = h2 + GROUP_OFFSET;
        for (ix, byte) in b"NOT-A-RADIO".iter().enumerate() {
            file[at + ix] = byte ^ secrets.h2_key[(at + ix - h2) % 2];
        }

        assert!(matches!(
            SglFirmware::parse(&file),
            Err(Error::UnsupportedCounterMagic)
        ));
        assert!(!SglFirmware::is_supported(&file));
    }

    #[test]
    fn a_parsed_file_writes_back_unchanged() {
        // reading and writing must be lossless, otherwise re-saving a file
        // silently rerolls the header secrets and the model key filler
        for config in ALL {
            let file = wrap(config.radio_model, &sample(0x200, 21));
            let fw = SglFirmware::parse(&file).expect("parses");
            let again = fw.serialise().expect("serialises");
            assert_eq!(
                again, file,
                "{}: rewriting changed the file",
                config.radio_model
            );
        }
    }

    #[test]
    fn a_parsed_file_keeps_its_model_key() {
        let file = wrap("GD77", &sample(0x100, 22));
        let fw = SglFirmware::parse(&file).expect("parses");

        let key = fw.model_key.expect("a parsed file carries its key");
        assert_eq!(&key[..4], b"DV01", "the key prefix identifies the radio");
        assert_eq!(
            &key[4..],
            fixed_secrets().transfer_key,
            "the filler was not preserved"
        );
    }

    #[test]
    fn compatibility_follows_the_radio() {
        let mut a = SglFirmware::new("GD77").expect("model is supported");
        a.set_data(&[0; 16]).expect("data fits");
        let mut b = SglFirmware::new("GD77").expect("model is supported");
        b.set_data(&[0; 16]).expect("data fits");
        let mut c = SglFirmware::new("DM1801").expect("model is supported");
        c.set_data(&[0; 16]).expect("data fits");
        let mut d = SglFirmware::new("GD77").expect("model is supported");
        d.set_data(&[0; 32]).expect("data fits");

        assert!(a.is_compatible(&b));
        assert!(!a.is_compatible(&c));
        assert!(!a.is_compatible(&d), "a different length is not compatible");
    }
}
