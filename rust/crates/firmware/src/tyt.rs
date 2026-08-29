//! The TYT firmware container, as used by the MD-380 family, MD-9600,
//! MD-UV3x0, MD-2017 and the Baofeng DM-1701.
//!
//! Layout:
//!
//! ```text
//! 0x000  16  "OutSecurityBin\0\0"
//! 0x010  16  firmware model, null padded
//! 0x020  16  four little endian u32 of unknown meaning
//! 0x030  76  counter magic, length is 1 + the first byte, then filler
//! 0x07c   4  region count
//! 0x080 128  region table, pairs of (address, length), 0xff filled
//! 0x100   n  firmware data, XOR encrypted, regions laid end to end
//!        240 0xff padding
//!         16 "OutputBinDataEnd"
//! ```

use crate::cipher;
use crate::{Error, Result, Segment};

/// Start magic, "OutSecurityBin\0\0"
const MAGIC_BEGIN: [u8; 16] = *b"OutSecurityBin\0\0";
/// End magic, "OutputBinDataEnd"
const MAGIC_END: [u8; 16] = *b"OutputBinDataEnd";

const HEADER_LEN: usize = 0x80;
const REGION_TABLE_LEN: usize = 0x80;
/// Where the firmware data starts, header plus region table
const DATA_OFFSET: usize = HEADER_LEN + REGION_TABLE_LEN;
const FOOTER_PAD: usize = 0x100 - 16;

const RADIO_FIELD_LEN: usize = 16;
const COUNTER_MAGIC_LEN: usize = 76;

/// Everything that varies between radios in this family
#[derive(Debug, Clone, Copy)]
pub struct RadioConfig {
    /// Model as the user names it on the command line
    pub radio_model: &'static str,
    /// Model string written into the firmware header
    pub firmware_model: &'static str,
    /// Identifies the radio, the first byte is the length of the rest
    pub counter_magic: &'static [u8],
    /// XOR key for the firmware data
    pub cipher: &'static [u8],
}

/// Every radio this container supports.
///
/// The order matters: `from_counter_magic` returns the first match, and
/// several MD-2017 variants share a firmware model.
pub const ALL: &[RadioConfig] = &[
    RadioConfig {
        radio_model: "MD2017", // REC
        firmware_model: "MD-9600",
        counter_magic: &[0x02, 0x19, 0x0c],
        cipher: cipher::UV3X0,
    },
    RadioConfig {
        radio_model: "MD2017 GPS", // REC
        firmware_model: "MD-9600",
        counter_magic: &[0x02, 0x18, 0x0c],
        cipher: cipher::UV3X0,
    },
    RadioConfig {
        radio_model: "MD2017", // CSV
        firmware_model: "MD-9600",
        counter_magic: &[0x01, 0x19],
        cipher: cipher::UV3X0,
    },
    RadioConfig {
        radio_model: "MD2017 GPS", // CSV
        firmware_model: "MD-9600",
        counter_magic: &[0x01, 0x18],
        cipher: cipher::UV3X0,
    },
    RadioConfig {
        radio_model: "MD9600",
        firmware_model: "MD-9600",
        counter_magic: &[0x01, 0x14],
        cipher: cipher::MD9600,
    },
    RadioConfig {
        radio_model: "UV3X0 GPS",
        firmware_model: "MD-9600",
        counter_magic: &[0x02, 0x16, 0x0c],
        cipher: cipher::UV3X0,
    },
    RadioConfig {
        radio_model: "UV3X0",
        firmware_model: "MD-9600",
        counter_magic: &[0x02, 0x17, 0x0c],
        cipher: cipher::UV3X0,
    },
    RadioConfig {
        radio_model: "DM1701",
        firmware_model: "DM1701",
        counter_magic: &[0x01, 0x0f],
        cipher: cipher::DM1701,
    },
    RadioConfig {
        radio_model: "MD390",
        firmware_model: "JST51",
        counter_magic: &[0x01, 0x10],
        cipher: cipher::MD380,
    },
    RadioConfig {
        radio_model: "MD380",
        firmware_model: "JST51",
        counter_magic: &[0x01, 0x0d],
        cipher: cipher::MD380,
    },
    RadioConfig {
        radio_model: "MD446",
        firmware_model: "JST51",
        counter_magic: &[0x01, 0x0d],
        cipher: cipher::MD380,
    },
    RadioConfig {
        radio_model: "MD280",
        firmware_model: "JST51",
        counter_magic: &[0x01, 0x1b],
        cipher: cipher::MD380,
    },
];

/// Look up a radio by the name used on the command line
pub fn config_for_model(model: &str) -> Option<&'static RadioConfig> {
    ALL.iter().find(|r| r.radio_model == model)
}

/// Look up a radio by the counter magic found in a firmware header
pub fn config_for_counter_magic(magic: &[u8]) -> Option<&'static RadioConfig> {
    ALL.iter().find(|r| r.counter_magic == magic)
}

/// A parsed TYT firmware file, holding decrypted firmware data
#[derive(Debug, Clone)]
pub struct TytFirmware {
    config: &'static RadioConfig,
    /// Decrypted firmware data, every region laid end to end
    data: Vec<u8>,
    /// (address, length) per region, lengths sum to `data.len()`
    regions: Vec<(u32, u32)>,
    /// The region count exactly as the file wrote it.
    ///
    /// Vendor firmware is inconsistent here: some single region files write 1
    /// and others leave the field as 0xffffffff, which means the same thing.
    /// Keeping the original makes reading and writing lossless.
    region_count_raw: Option<u32>,
}

impl TytFirmware {
    /// Start a new firmware file for a radio
    pub fn new(model: &str) -> Result<Self> {
        let config =
            config_for_model(model).ok_or_else(|| Error::UnknownModel(model.to_owned()))?;
        Ok(Self {
            config,
            data: Vec::new(),
            regions: Vec::new(),
            region_count_raw: None,
        })
    }

    /// The radio this firmware is for
    pub fn config(&self) -> &'static RadioConfig {
        self.config
    }

    /// Decrypted firmware data, every region laid end to end
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The firmware split back into the regions it is written to
    pub fn segments(&self) -> Vec<Segment<'_>> {
        let mut out = Vec::with_capacity(self.regions.len());
        let mut offset = 0usize;
        for (address, length) in &self.regions {
            let len = *length as usize;
            // the parser guarantees the regions fit, so a short read here
            // would be a bug rather than bad input
            let end = offset.saturating_add(len).min(self.data.len());
            let Some(data) = self.data.get(offset..end) else {
                break;
            };
            out.push(Segment {
                address: *address,
                data,
            });
            offset = end;
        }
        out
    }

    /// The firmware as it is stored in the file, still enciphered.
    ///
    /// This is what a radio is sent. Its bootloader takes the stored form and
    /// deciphers it itself, so writing the deciphered bytes gives a radio
    /// that accepts every block, reports success, and then will not start.
    /// The C++ does the same thing by never deciphering on the way to the
    /// radio: only `--unwrap` asks for that.
    pub fn data_as_stored(&self) -> Vec<u8> {
        let mut data = self.data.clone();
        cipher::apply_xor(&mut data, self.config.cipher, 0);
        data
    }

    /// The stored form, split into the regions it is written to
    pub fn segments_as_stored(&self) -> Vec<(u32, Vec<u8>)> {
        let stored = self.data_as_stored();
        let mut out = Vec::with_capacity(self.regions.len());
        let mut offset = 0usize;

        for (address, length) in &self.regions {
            let end = offset.saturating_add(*length as usize).min(stored.len());
            let Some(part) = stored.get(offset..end) else {
                break;
            };
            out.push((*address, part.to_vec()));
            offset = end;
        }
        out
    }

    /// Append a region of firmware, padding it out to `align` bytes with 0xff
    /// when `align` is not zero
    pub fn append_segment(&mut self, address: u32, data: &[u8], align: usize) -> Result<()> {
        let padded_len = match align {
            0 => data.len(),
            a => data.len().next_multiple_of(a),
        };
        let length = u32::try_from(padded_len).map_err(|_| Error::SegmentTooLarge(padded_len))?;

        if (self.regions.len() + 1) * 8 > REGION_TABLE_LEN {
            return Err(Error::TooManyRegions(self.regions.len() + 1));
        }

        self.data.extend_from_slice(data);
        self.data
            .resize(self.data.len() + (padded_len - data.len()), 0xff);
        self.regions.push((address, length));
        Ok(())
    }

    /// Is this the same radio and firmware model as `other`
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.config.radio_model == other.config.radio_model
            && self.config.firmware_model == other.config.firmware_model
    }

    /// Read a firmware file
    pub fn parse(input: &[u8]) -> Result<Self> {
        let header = input.get(..HEADER_LEN).ok_or(Error::Truncated {
            what: "header",
            wanted: HEADER_LEN,
            got: input.len(),
        })?;

        let magic = header.get(..16).ok_or(Error::Malformed("header magic"))?;
        if magic != MAGIC_BEGIN {
            return Err(Error::BadMagic);
        }

        let counter_magic = read_counter_magic(header)?;
        let config =
            config_for_counter_magic(counter_magic).ok_or(Error::UnsupportedCounterMagic)?;

        let raw_region_count = read_u32(header, 0x7c).ok_or(Error::Malformed("region count"))?;
        let mut n_regions = raw_region_count;
        if n_regions == u32::MAX {
            // 0xffffffff means one region
            n_regions = 1;
        }
        let n_regions = n_regions as usize;
        if n_regions * 8 > REGION_TABLE_LEN {
            return Err(Error::TooManyRegions(n_regions));
        }

        let table = input.get(HEADER_LEN..DATA_OFFSET).ok_or(Error::Truncated {
            what: "region table",
            wanted: DATA_OFFSET,
            got: input.len(),
        })?;

        let mut regions = Vec::with_capacity(n_regions);
        let mut total = 0usize;
        for ix in 0..n_regions {
            let start = read_u32(table, ix * 8).ok_or(Error::Malformed("region address"))?;
            let length = read_u32(table, (ix * 8) + 4).ok_or(Error::Malformed("region length"))?;
            total = total
                .checked_add(length as usize)
                .ok_or(Error::Malformed("region lengths overflow"))?;
            regions.push((start, length));
        }

        let end = DATA_OFFSET
            .checked_add(total)
            .ok_or(Error::Malformed("firmware length overflow"))?;
        let mut data = input
            .get(DATA_OFFSET..end)
            .ok_or(Error::Truncated {
                what: "firmware data",
                wanted: end,
                got: input.len(),
            })?
            .to_vec();

        cipher::apply_xor(&mut data, config.cipher, 0);

        Ok(Self {
            config,
            data,
            regions,
            region_count_raw: Some(raw_region_count),
        })
    }

    /// Write a firmware file
    pub fn serialise(&self) -> Result<Vec<u8>> {
        if self.regions.is_empty() {
            return Err(Error::NoSegments);
        }
        if self.regions.len() * 8 > REGION_TABLE_LEN {
            return Err(Error::TooManyRegions(self.regions.len()));
        }
        if self.config.firmware_model.len() > RADIO_FIELD_LEN {
            return Err(Error::Malformed("firmware model does not fit the header"));
        }

        let mut out = Vec::with_capacity(DATA_OFFSET + self.data.len() + 0x100);

        out.extend_from_slice(&MAGIC_BEGIN);

        // Genuine vendor firmware writes the model, two nulls, then 0xff to
        // the end of the field, and md380tools reproduces that. radio_tool
        // zero filled instead, which no real firmware file does.
        let mut radio = [0xffu8; RADIO_FIELD_LEN];
        let model = self.config.firmware_model.as_bytes();
        for (slot, byte) in radio.iter_mut().zip(model) {
            *slot = *byte;
        }
        for slot in radio.iter_mut().skip(model.len()).take(2) {
            *slot = 0x00;
        }
        out.extend_from_slice(&radio);

        // n1, n2 are fixed values the official tool writes, n3 and n4 are zero
        out.extend_from_slice(&0x3000_0230u32.to_le_bytes());
        out.extend_from_slice(&0x4700_4000u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());

        let mut counter = [0u8; COUNTER_MAGIC_LEN];
        for (ix, slot) in counter.iter_mut().enumerate() {
            *slot = if ix > 0x20 { 0xff } else { ix as u8 };
        }
        for (slot, byte) in counter.iter_mut().zip(self.config.counter_magic) {
            *slot = *byte;
        }
        out.extend_from_slice(&counter);

        let n_regions = u32::try_from(self.regions.len())
            .map_err(|_| Error::TooManyRegions(self.regions.len()))?;
        // write back exactly what was read, so a parsed file round trips
        let written = match self.region_count_raw {
            Some(raw) if raw == u32::MAX && n_regions == 1 => raw,
            _ => n_regions,
        };
        out.extend_from_slice(&written.to_le_bytes());
        debug_assert_eq!(out.len(), HEADER_LEN);

        for (address, length) in &self.regions {
            out.extend_from_slice(&address.to_le_bytes());
            out.extend_from_slice(&length.to_le_bytes());
        }
        out.resize(DATA_OFFSET, 0xff);

        let mut data = self.data.clone();
        cipher::apply_xor(&mut data, self.config.cipher, 0);
        out.extend_from_slice(&data);

        out.resize(out.len() + FOOTER_PAD, 0xff);
        out.extend_from_slice(&MAGIC_END);

        Ok(out)
    }

    /// Does this look like a TYT firmware file
    pub fn is_supported(input: &[u8]) -> bool {
        let Some(header) = input.get(..HEADER_LEN) else {
            return false;
        };
        if header.get(..16) != Some(&MAGIC_BEGIN[..]) {
            return false;
        }
        let Ok(magic) = read_counter_magic(header) else {
            return false;
        };
        config_for_counter_magic(magic).is_some()
    }
}

/// The encrypted firmware exactly as it sits in the file.
///
/// Useful when the key is not known, which is the case for a radio nobody has
/// looked at yet: feed this to [`crate::keyguess::guess_key`].
pub fn encrypted_payload(input: &[u8]) -> Result<&[u8]> {
    let end = input.len();
    input.get(DATA_OFFSET..end).ok_or(Error::Truncated {
        what: "firmware data",
        wanted: DATA_OFFSET,
        got: input.len(),
    })
}

/// The counter magic is prefixed with the length of the bytes that follow it
fn read_counter_magic(header: &[u8]) -> Result<&[u8]> {
    let field = header
        .get(0x30..0x30 + COUNTER_MAGIC_LEN)
        .ok_or(Error::Malformed("counter magic"))?;
    let len = *field.first().ok_or(Error::Malformed("counter magic"))? as usize;
    if len > 3 {
        return Err(Error::Malformed("counter magic length"));
    }
    field
        .get(..len + 1)
        .ok_or(Error::Malformed("counter magic"))
}

fn read_u32(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([
        *bytes.first()?,
        *bytes.get(1)?,
        *bytes.get(2)?,
        *bytes.get(3)?,
    ]))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod stored_form_tests {
    use super::*;

    /// The stored form has to be exactly the bytes in the file, because that
    /// is what goes to the radio. Deciphering on the way out is what left a
    /// DM-1701 unbootable after a flash that reported success.
    #[test]
    fn the_stored_form_is_the_bytes_from_the_file() {
        let mut fw = TytFirmware::new("DM1701").expect("a known radio");
        let body: Vec<u8> = (0..0x2000u32).map(|i| (i % 251) as u8).collect();
        fw.append_segment(0x0800_c000, &body, 0).expect("appends");

        let file = fw.serialise().expect("serialises");
        let parsed = TytFirmware::parse(&file).expect("parses");

        // what the radio is sent
        let stored = parsed.segments_as_stored();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].0, 0x0800_c000);

        // and it is byte for byte the data section of the file
        let from_file = &file[DATA_OFFSET..DATA_OFFSET + body.len()];
        assert_eq!(
            stored[0].1, from_file,
            "the stored form must match the file, not the deciphered image"
        );
    }

    #[test]
    fn the_stored_form_differs_from_the_deciphered_one() {
        let mut fw = TytFirmware::new("DM1701").expect("a known radio");
        let body = vec![0u8; 0x400];
        fw.append_segment(0x0800_c000, &body, 0).expect("appends");
        let file = fw.serialise().expect("serialises");
        let parsed = TytFirmware::parse(&file).expect("parses");

        let plain = parsed.segments();
        let stored = parsed.segments_as_stored();

        assert_ne!(
            plain[0].data,
            stored[0].1.as_slice(),
            "if these were the same the cipher would be doing nothing"
        );
    }

    #[test]
    fn the_stored_form_deciphers_back_to_the_image() {
        let mut fw = TytFirmware::new("DM1701").expect("a known radio");
        let body: Vec<u8> = (0..0x1000u32).map(|i| (i % 97) as u8).collect();
        fw.append_segment(0x0800_c000, &body, 0).expect("appends");
        let file = fw.serialise().expect("serialises");
        let parsed = TytFirmware::parse(&file).expect("parses");

        let mut stored = parsed.segments_as_stored()[0].1.clone();
        cipher::apply_xor(&mut stored, parsed.config().cipher, 0);
        assert_eq!(stored, body, "the cipher is its own inverse");
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

    fn wrap(model: &str, segments: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut fw = TytFirmware::new(model).expect("model is supported");
        for (address, data) in segments {
            fw.append_segment(*address, data, 0).expect("segment fits");
        }
        fw.serialise().expect("serialises")
    }

    #[test]
    fn every_config_has_a_sane_counter_magic() {
        for config in ALL {
            let len = config.counter_magic.len();
            assert!((2..=4).contains(&len), "{}: odd magic", config.radio_model);
            assert_eq!(
                config.counter_magic.first().copied(),
                Some((len - 1) as u8),
                "{}: first byte must be the length of the rest",
                config.radio_model
            );
            assert!(
                config.firmware_model.len() <= RADIO_FIELD_LEN,
                "{}: firmware model does not fit the header",
                config.radio_model
            );
            assert!(!config.cipher.is_empty());
        }
    }

    #[test]
    fn radios_that_share_a_counter_magic_are_indistinguishable() {
        // MD380 and MD446 carry the same magic, so a file for one reads back
        // as the other. The C++ behaves identically. This is a property of
        // the format, not a bug to be fixed here: do not "correct" it without
        // a way to tell the two apart.
        let md380 = config_for_model("MD380").expect("model is in the table");
        let md446 = config_for_model("MD446").expect("model is in the table");
        assert_eq!(md380.counter_magic, md446.counter_magic);

        let resolved = config_for_counter_magic(md446.counter_magic).expect("magic resolves");
        assert_eq!(resolved.radio_model, "MD380", "the first match wins");
    }

    #[test]
    fn model_lookup_round_trips() {
        for config in ALL {
            let found = config_for_model(config.radio_model).expect("model is in the table");
            // several models share a counter magic, so only check the magic resolves
            assert!(config_for_counter_magic(found.counter_magic).is_some());
        }
        assert!(config_for_model("NOT-A-RADIO").is_none());
    }

    #[test]
    fn single_segment_round_trip() {
        let data = sample(0x4000, 1);
        let file = wrap("DM1701", &[(0x0800_c000, data.clone())]);

        let fw = TytFirmware::parse(&file).expect("parses");
        assert_eq!(fw.config().radio_model, "DM1701");

        let segments = fw.segments();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].address, 0x0800_c000);
        assert_eq!(segments[0].data, data);
    }

    #[test]
    fn multi_segment_round_trip() {
        let a = sample(0x2000, 2);
        let b = sample(0x1000, 3);
        let file = wrap(
            "UV3X0",
            &[(0x0800_c000, a.clone()), (0x0804_0000, b.clone())],
        );

        let fw = TytFirmware::parse(&file).expect("parses");
        let segments = fw.segments();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].address, 0x0800_c000);
        assert_eq!(segments[0].data, a);
        assert_eq!(segments[1].address, 0x0804_0000);
        assert_eq!(segments[1].data, b);
    }

    #[test]
    fn serialise_is_stable() {
        // the same input must always produce the same file, there is nothing
        // random in this container
        let data = sample(0x800, 4);
        assert_eq!(
            wrap("MD9600", &[(0x0800_c000, data.clone())]),
            wrap("MD9600", &[(0x0800_c000, data)])
        );
    }

    #[test]
    fn file_layout_is_what_the_radio_expects() {
        let file = wrap("DM1701", &[(0x0800_c000, sample(0x200, 5))]);

        assert_eq!(&file[..16], &MAGIC_BEGIN);
        assert_eq!(&file[16..21], b"DM170");
        assert_eq!(&file[file.len() - 16..], &MAGIC_END);
        // header, region table, data, footer
        assert_eq!(file.len(), DATA_OFFSET + 0x200 + FOOTER_PAD + 16);
        // the unused part of the region table is 0xff filled
        assert!(file[HEADER_LEN + 8..DATA_OFFSET].iter().all(|b| *b == 0xff));
    }

    #[test]
    fn data_on_disk_is_encrypted() {
        let plain = vec![0u8; 0x400];
        let file = wrap("DM1701", &[(0x0800_c000, plain.clone())]);
        let on_disk = &file[DATA_OFFSET..DATA_OFFSET + plain.len()];
        assert_ne!(
            on_disk,
            &plain[..],
            "firmware data was written in the clear"
        );
        assert_eq!(on_disk, &cipher::DM1701[..0x400]);
    }

    #[test]
    fn alignment_pads_with_ff() {
        let mut fw = TytFirmware::new("DM1701").expect("model is supported");
        fw.append_segment(0x0800_c000, &[1, 2, 3], 0x200)
            .expect("segment fits");

        let segments = fw.segments();
        assert_eq!(segments[0].data.len(), 0x200);
        assert_eq!(&segments[0].data[..3], &[1, 2, 3]);
        assert!(segments[0].data[3..].iter().all(|b| *b == 0xff));
    }

    #[test]
    fn compatibility_follows_the_radio() {
        let dm = TytFirmware::new("DM1701").expect("model is supported");
        let dm2 = TytFirmware::new("DM1701").expect("model is supported");
        let md = TytFirmware::new("MD9600").expect("model is supported");

        assert!(dm.is_compatible(&dm2));
        assert!(!dm.is_compatible(&md));
        assert!(!md.is_compatible(&dm));
    }

    #[test]
    fn unknown_model_is_rejected() {
        assert!(matches!(
            TytFirmware::new("NOT-A-RADIO"),
            Err(Error::UnknownModel(_))
        ));
    }

    #[test]
    fn empty_firmware_will_not_serialise() {
        let fw = TytFirmware::new("DM1701").expect("model is supported");
        assert!(matches!(fw.serialise(), Err(Error::NoSegments)));
    }

    #[test]
    fn too_many_regions_is_rejected() {
        let mut fw = TytFirmware::new("DM1701").expect("model is supported");
        for ix in 0..16 {
            fw.append_segment(ix * 0x1000, &[0u8; 16], 0)
                .expect("the first 16 regions fit");
        }
        assert!(matches!(
            fw.append_segment(0x9_0000, &[0u8; 16], 0),
            Err(Error::TooManyRegions(17))
        ));
    }

    #[test]
    fn malformed_input_never_panics() {
        let good = wrap("DM1701", &[(0x0800_c000, sample(0x400, 6))]);

        // every truncation of a valid file
        for len in 0..good.len() {
            let _ = TytFirmware::parse(&good[..len]);
            let _ = TytFirmware::is_supported(&good[..len]);
        }

        // wrong magic
        let mut bad = good.clone();
        bad[0] = b'X';
        assert!(matches!(TytFirmware::parse(&bad), Err(Error::BadMagic)));
        assert!(!TytFirmware::is_supported(&bad));

        // counter magic that claims to be longer than the field allows
        let mut bad = good.clone();
        bad[0x30] = 0xff;
        assert!(TytFirmware::parse(&bad).is_err());

        // counter magic no radio uses
        let mut bad = good.clone();
        bad[0x31] = 0xaa;
        assert!(matches!(
            TytFirmware::parse(&bad),
            Err(Error::UnsupportedCounterMagic)
        ));

        // a region count past the end of the table
        let mut bad = good.clone();
        bad[0x7c..0x80].copy_from_slice(&64u32.to_le_bytes());
        assert!(matches!(
            TytFirmware::parse(&bad),
            Err(Error::TooManyRegions(64))
        ));

        // a region length that runs past the end of the file
        let mut bad = good.clone();
        bad[HEADER_LEN + 4..HEADER_LEN + 8].copy_from_slice(&0x00ff_ffffu32.to_le_bytes());
        assert!(matches!(
            TytFirmware::parse(&bad),
            Err(Error::Truncated { .. })
        ));

        // region lengths chosen to overflow a usize when summed
        let mut bad = good.clone();
        bad[0x7c..0x80].copy_from_slice(&2u32.to_le_bytes());
        bad[HEADER_LEN + 4..HEADER_LEN + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        bad[HEADER_LEN + 12..HEADER_LEN + 16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(TytFirmware::parse(&bad).is_err());

        // random noise of every length up to a full header
        for len in 0..0x120 {
            let noise = sample(len, (len as u32) + 1);
            let _ = TytFirmware::parse(&noise);
            let _ = TytFirmware::is_supported(&noise);
        }
    }

    #[test]
    fn region_count_of_ffffffff_means_one() {
        let mut file = wrap("DM1701", &[(0x0800_c000, sample(0x400, 7))]);
        file[0x7c..0x80].copy_from_slice(&u32::MAX.to_le_bytes());

        let fw = TytFirmware::parse(&file).expect("parses");
        assert_eq!(fw.segments().len(), 1);
    }
}
