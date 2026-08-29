//! The Connect Systems firmware container, used by the CS800 and CS800D.
//!
//! Layout:
//!
//! ```text
//! 0x00   4  base address offset, added to 0x08000000
//! 0x04   8  two unknown addresses, zero in every file seen
//! 0x0c   4  resource size, zero for a firmware image
//! 0x10   4  unknown, zero
//! 0x14   4  image size
//! 0x18  36  padding
//! 0x3c   4  resource header size, zero for a firmware image
//! 0x40   4  unknown header size, zero
//! 0x44   4  image header size, always 0x80
//! 0x48  36  padding
//! 0x6c   4  version, 1
//! 0x70  16  reserved
//! 0x80   n  firmware data, XOR encrypted
//!        2  checksum, itself XOR encrypted
//! ```
//!
//! The checksum covers the header followed by the *decrypted* firmware, and
//! is then obfuscated with two bytes of the same key.
//!
//! # Two dialects
//!
//! CSFWTOOL, which is where radio_tool got this format, writes the header and
//! the encrypted firmware and stops. radio_tool appends the two checksum
//! bytes and refuses to read a file without them. Their headers and payloads
//! are otherwise byte identical, verified by building CSFWTOOL and comparing.
//!
//! radio_tool is probably right that real firmware carries the checksum,
//! because its size check would otherwise reject every vendor file. Its own
//! source carries the comment "checksum not working right now", so this has
//! not been settled against a real radio.
//!
//! This port writes the checksum, matching radio_tool, and reads files with
//! or without one. Reading is harmless either way, and refusing a CSFWTOOL
//! file for a trailing field nobody is sure about helps nobody.

use crate::cipher;
use crate::{Error, Result, Segment};

/// Size of the header, also the value the header carries at 0x44
pub const HEADER_LEN: usize = 0x80;

const BASE_ADDR_AT: usize = 0x00;
const RSRC_SIZE_AT: usize = 0x0c;
const IMAGE_SIZE_AT: usize = 0x14;
const RSRC_HEADER_SIZE_AT: usize = 0x3c;
const IMAGE_HEADER_SIZE_AT: usize = 0x44;
const VERSION_AT: usize = 0x6c;

/// The version the writer stamps on a file
const VERSION: u32 = 1;

/// The only radio this container is known to belong to
pub const RADIO_MODEL: &str = "CS800";

/// Is this a radio this container supports
pub fn supports_model(model: &str) -> bool {
    model == RADIO_MODEL
}

/// A parsed Connect Systems firmware file, holding decrypted firmware data
#[derive(Debug, Clone)]
pub struct CsFirmware {
    /// Where the firmware is written, added to 0x08000000 by the radio
    base_address: u32,
    /// Decrypted firmware data
    data: Vec<u8>,
    /// Whether the file this came from carried a checksum
    had_checksum: bool,
}

impl CsFirmware {
    /// Start a new firmware file
    pub fn new(model: &str) -> Result<Self> {
        if !supports_model(model) {
            return Err(Error::UnknownModel(model.to_owned()));
        }
        Ok(Self {
            base_address: 0,
            data: Vec::new(),
            had_checksum: true,
        })
    }

    /// Did the file this was read from carry a trailing checksum. A file
    /// written by CSFWTOOL does not. Always true for one we built ourselves,
    /// because [`Self::serialise`] always writes one.
    pub fn had_checksum(&self) -> bool {
        self.had_checksum
    }

    /// Decrypted firmware data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Where the firmware is written on the device
    pub fn base_address(&self) -> u32 {
        self.base_address
    }

    /// This container holds a single region
    pub fn segments(&self) -> Vec<Segment<'_>> {
        vec![Segment {
            address: self.base_address,
            data: &self.data,
        }]
    }

    /// Set the firmware data and the address it is written to
    pub fn set_segment(&mut self, address: u32, data: &[u8]) -> Result<()> {
        u32::try_from(data.len()).map_err(|_| Error::SegmentTooLarge(data.len()))?;
        self.base_address = address;
        self.data = data.to_vec();
        self.had_checksum = true;
        Ok(())
    }

    /// There is no way to tell two CS firmware files apart yet, so any other
    /// CS firmware counts as compatible
    pub fn is_compatible(&self, _other: &Self) -> bool {
        true
    }

    /// Read a firmware file
    pub fn parse(input: &[u8]) -> Result<Self> {
        let header = input.get(..HEADER_LEN).ok_or(Error::Truncated {
            what: "header",
            wanted: HEADER_LEN,
            got: input.len(),
        })?;

        let image_size = read_u32(header, IMAGE_SIZE_AT).ok_or(Error::Malformed("image size"))?;
        if image_size == 0 {
            // a resource file rather than a firmware image
            return Err(Error::Malformed("image size is zero"));
        }

        let header_size =
            read_u32(header, IMAGE_HEADER_SIZE_AT).ok_or(Error::Malformed("image header size"))?;
        if header_size as usize != HEADER_LEN {
            return Err(Error::Malformed("image header size is not 0x80"));
        }

        let body_end = HEADER_LEN
            .checked_add(image_size as usize)
            .ok_or(Error::Malformed("image size overflow"))?;
        let with_checksum = body_end
            .checked_add(2)
            .ok_or(Error::Malformed("image size overflow"))?;

        // radio_tool writes a trailing checksum, CSFWTOOL does not
        let had_checksum = if input.len() == with_checksum {
            true
        } else if input.len() == body_end {
            false
        } else {
            return Err(Error::Malformed(
                "file size does not match the size in the header",
            ));
        };
        let wanted = with_checksum;

        let base_address =
            read_u32(header, BASE_ADDR_AT).ok_or(Error::Malformed("base address"))?;

        let mut data = input
            .get(HEADER_LEN..body_end)
            .ok_or(Error::Truncated {
                what: "firmware data",
                wanted,
                got: input.len(),
            })?
            .to_vec();

        // the checksum covers the header and the decrypted firmware
        cipher::apply_xor(&mut data, cipher::CS800_0, 0);

        if had_checksum {
            let stored = input
                .get(body_end..with_checksum)
                .and_then(|b| Some(u16::from_le_bytes([*b.first()?, *b.get(1)?])))
                .ok_or(Error::Malformed("checksum"))?;
            let stored = unmask_checksum(stored, image_size);

            let want = checksum(header, &data);
            if stored != want {
                return Err(Error::BadChecksum {
                    stored,
                    computed: want,
                });
            }
        }

        Ok(Self {
            base_address,
            data,
            had_checksum,
        })
    }

    /// Write a firmware file
    pub fn serialise(&self) -> Result<Vec<u8>> {
        if self.data.is_empty() {
            return Err(Error::NoSegments);
        }
        let image_size =
            u32::try_from(self.data.len()).map_err(|_| Error::SegmentTooLarge(self.data.len()))?;

        let mut header = [0u8; HEADER_LEN];
        write_u32(&mut header, BASE_ADDR_AT, self.base_address)?;
        write_u32(&mut header, RSRC_SIZE_AT, 0)?;
        write_u32(&mut header, IMAGE_SIZE_AT, image_size)?;
        write_u32(&mut header, RSRC_HEADER_SIZE_AT, 0)?;
        write_u32(&mut header, IMAGE_HEADER_SIZE_AT, HEADER_LEN as u32)?;
        write_u32(&mut header, VERSION_AT, VERSION)?;

        let sum = checksum(&header, &self.data);

        let mut out = Vec::with_capacity(HEADER_LEN + self.data.len() + 2);
        out.extend_from_slice(&header);

        let mut data = self.data.clone();
        cipher::apply_xor(&mut data, cipher::CS800_0, 0);
        out.extend_from_slice(&data);

        out.extend_from_slice(&unmask_checksum(sum, image_size).to_le_bytes());
        Ok(out)
    }

    /// Does this look like a Connect Systems firmware file
    pub fn is_supported(input: &[u8]) -> bool {
        let Some(header) = input.get(..HEADER_LEN) else {
            return false;
        };
        let Some(image_size) = read_u32(header, IMAGE_SIZE_AT) else {
            return false;
        };
        if image_size == 0 {
            return false;
        }
        if read_u32(header, IMAGE_HEADER_SIZE_AT) != Some(HEADER_LEN as u32) {
            return false;
        }
        let Some(body_end) = HEADER_LEN.checked_add(image_size as usize) else {
            return false;
        };
        // with a checksum, as radio_tool writes, or without, as CSFWTOOL does
        input.len() == body_end || Some(input.len()) == body_end.checked_add(2)
    }
}

/// Sum every byte, divide by five, then swap the two halves.
///
/// This is what the C++ calls CSChecksum. The division and the swap look
/// wrong, and they probably are, but the radio expects exactly this.
fn checksum(header: &[u8], plain_data: &[u8]) -> u16 {
    let mut sum: u16 = 0;
    for byte in header.iter().chain(plain_data) {
        sum = sum.wrapping_add(u16::from(*byte));
    }
    let fifth = sum / 5;
    let c0 = fifth >> 8;
    let c1 = fifth & 0xff;
    (c1 << 8) | c0
}

/// The checksum on disk is obfuscated with two bytes of the cipher, chosen by
/// the image size. Its own inverse.
fn unmask_checksum(value: u16, image_size: u32) -> u16 {
    let key = cipher::CS800_0;
    if key.is_empty() {
        return value;
    }
    let size = image_size as usize;
    let lo_at = size % key.len();
    let hi_at = size.wrapping_add(1) % key.len();
    let lo = key.get(lo_at).copied().unwrap_or(0);
    let hi = key.get(hi_at).copied().unwrap_or(0);

    let bytes = value.to_le_bytes();
    u16::from_le_bytes([
        bytes.first().copied().unwrap_or(0) ^ lo,
        bytes.get(1).copied().unwrap_or(0) ^ hi,
    ])
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

fn write_u32(buf: &mut [u8], offset: usize, value: u32) -> Result<()> {
    let slot = buf
        .get_mut(offset..offset + 4)
        .ok_or(Error::Malformed("header field does not fit"))?;
    slot.copy_from_slice(&value.to_le_bytes());
    Ok(())
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

    fn wrap(address: u32, data: &[u8]) -> Vec<u8> {
        let mut fw = CsFirmware::new(RADIO_MODEL).expect("model is supported");
        fw.set_segment(address, data).expect("data fits");
        fw.serialise().expect("serialises")
    }

    #[test]
    fn round_trip_through_a_file() {
        let data = sample(0x2000, 1);
        let file = wrap(0x0002_0000, &data);

        assert!(CsFirmware::is_supported(&file));

        let fw = CsFirmware::parse(&file).expect("parses");
        assert_eq!(fw.data(), data);
        assert_eq!(fw.base_address(), 0x0002_0000);
        assert_eq!(fw.segments().len(), 1);
        assert_eq!(fw.segments()[0].address, 0x0002_0000);
    }

    #[test]
    fn file_layout_is_what_the_radio_expects() {
        let data = sample(0x400, 2);
        let file = wrap(0x0001_0000, &data);

        assert_eq!(file.len(), HEADER_LEN + data.len() + 2);
        assert_eq!(read_u32(&file, BASE_ADDR_AT), Some(0x0001_0000));
        assert_eq!(read_u32(&file, IMAGE_SIZE_AT), Some(0x400));
        assert_eq!(read_u32(&file, IMAGE_HEADER_SIZE_AT), Some(0x80));
        assert_eq!(read_u32(&file, VERSION_AT), Some(1));
        assert_eq!(read_u32(&file, RSRC_SIZE_AT), Some(0));
        assert_eq!(read_u32(&file, RSRC_HEADER_SIZE_AT), Some(0));
    }

    #[test]
    fn data_on_disk_is_encrypted() {
        let plain = vec![0u8; 0x100];
        let file = wrap(0, &plain);
        let on_disk = &file[HEADER_LEN..HEADER_LEN + plain.len()];
        assert_ne!(on_disk, &plain[..]);
        // the key repeats every 256 bytes
        assert_eq!(on_disk, cipher::CS800_0);
    }

    #[test]
    fn serialise_is_stable() {
        let data = sample(0x200, 3);
        assert_eq!(wrap(0x20000, &data), wrap(0x20000, &data));
    }

    #[test]
    fn checksum_masking_is_its_own_inverse() {
        for size in [0u32, 1, 255, 256, 257, 0x2000, u32::MAX] {
            for value in [0u16, 1, 0x1234, 0xffff] {
                assert_eq!(unmask_checksum(unmask_checksum(value, size), size), value);
            }
        }
    }

    #[test]
    fn a_corrupt_checksum_is_caught() {
        let mut file = wrap(0x20000, &sample(0x200, 4));
        let last = file.len() - 1;
        file[last] ^= 0xff;

        assert!(matches!(
            CsFirmware::parse(&file),
            Err(Error::BadChecksum { .. })
        ));
    }

    #[test]
    fn corrupt_firmware_data_is_caught() {
        let mut file = wrap(0x20000, &sample(0x200, 5));
        file[HEADER_LEN + 10] ^= 0xff;

        assert!(matches!(
            CsFirmware::parse(&file),
            Err(Error::BadChecksum { .. })
        ));
    }

    #[test]
    fn a_resource_file_is_not_a_firmware_image() {
        // image size of zero marks a resource file
        let mut file = wrap(0, &sample(0x100, 6));
        file[IMAGE_SIZE_AT..IMAGE_SIZE_AT + 4].copy_from_slice(&0u32.to_le_bytes());

        assert!(!CsFirmware::is_supported(&file));
        assert!(CsFirmware::parse(&file).is_err());
    }

    #[test]
    fn a_size_that_disagrees_with_the_file_is_rejected() {
        let mut file = wrap(0, &sample(0x100, 7));
        file[IMAGE_SIZE_AT..IMAGE_SIZE_AT + 4].copy_from_slice(&0x1000u32.to_le_bytes());

        assert!(!CsFirmware::is_supported(&file));
        assert!(CsFirmware::parse(&file).is_err());
    }

    #[test]
    fn unknown_model_is_rejected() {
        assert!(matches!(
            CsFirmware::new("NOT-A-RADIO"),
            Err(Error::UnknownModel(_))
        ));
        assert!(supports_model("CS800"));
        assert!(!supports_model("DM1701"));
    }

    #[test]
    fn empty_firmware_will_not_serialise() {
        let fw = CsFirmware::new(RADIO_MODEL).expect("model is supported");
        assert!(matches!(fw.serialise(), Err(Error::NoSegments)));
    }

    #[test]
    fn malformed_input_never_panics() {
        let good = wrap(0x20000, &sample(0x400, 8));

        for len in 0..good.len() {
            let _ = CsFirmware::parse(&good[..len]);
            let _ = CsFirmware::is_supported(&good[..len]);
        }

        for at in 0..HEADER_LEN {
            let mut bad = good.clone();
            bad[at] ^= 0xff;
            let _ = CsFirmware::parse(&bad);
            let _ = CsFirmware::is_supported(&bad);
        }

        // an image size chosen to overflow when the header and checksum are added
        let mut bad = good.clone();
        bad[IMAGE_SIZE_AT..IMAGE_SIZE_AT + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(CsFirmware::parse(&bad).is_err());

        for len in 0..0x100 {
            let noise = sample(len, (len as u32) + 1);
            let _ = CsFirmware::parse(&noise);
            let _ = CsFirmware::is_supported(&noise);
        }
    }
}
