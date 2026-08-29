//! Working out what a file is, and describing it.

use firmware::{Segment, ailunce, cs, sgl, tyt, yaesu};
use std::fmt::Write as _;

/// Every firmware container, once a file has been recognised as one
pub enum Firmware {
    /// TYT and Baofeng DM-1701
    Tyt(Box<tyt::TytFirmware>),
    /// Radioddity GD-77 and friends
    Sgl(Box<sgl::SglFirmware>),
    /// Connect Systems
    Cs(Box<cs::CsFirmware>),
    /// Ailunce HD1
    Ailunce(Box<ailunce::AilunceFirmware>),
    /// Yaesu FT-70D
    Yaesu(Box<yaesu::YaesuFirmware>),
}

impl Firmware {
    /// Read a file as the container a named radio uses.
    ///
    /// The Ailunce and Yaesu containers have no header and no magic, so a
    /// file can only be read as one of them if the user says so. The C++
    /// guessed, which is why any unrecognised file came out as Yaesu
    /// firmware.
    pub fn parse_as(data: &[u8], radio: &str) -> Result<Self, String> {
        if ailunce::supports_model(radio) {
            return ailunce::AilunceFirmware::parse(data)
                .map(|f| Self::Ailunce(Box::new(f)))
                .map_err(|e| e.to_string());
        }
        if yaesu::supports_model(radio) {
            return yaesu::YaesuFirmware::parse(data)
                .map(|f| Self::Yaesu(Box::new(f)))
                .map_err(|e| e.to_string());
        }

        // for the rest the file identifies itself, so just check it agrees
        let found = Self::identify(data)
            .ok_or_else(|| format!("this file is not a container that {radio} uses"))?;
        if found.radio() != radio {
            return Err(format!(
                "this file is {} firmware, not {radio}",
                found.radio()
            ));
        }
        Ok(found)
    }

    /// Identify a file by its contents.
    ///
    /// The containers with a header are tried first, because the two
    /// headerless ones will accept anything at all and would otherwise claim
    /// every file handed to them.
    pub fn identify(data: &[u8]) -> Option<Self> {
        if tyt::TytFirmware::is_supported(data) {
            return tyt::TytFirmware::parse(data)
                .ok()
                .map(|f| Self::Tyt(Box::new(f)));
        }
        if sgl::SglFirmware::is_supported(data) {
            return sgl::SglFirmware::parse(data)
                .ok()
                .map(|f| Self::Sgl(Box::new(f)));
        }
        if cs::CsFirmware::is_supported(data) {
            return cs::CsFirmware::parse(data)
                .ok()
                .map(|f| Self::Cs(Box::new(f)));
        }
        None
    }

    /// The radio this firmware is for, as far as the file says
    pub fn radio(&self) -> String {
        match self {
            Self::Tyt(f) => f.config().radio_model.to_owned(),
            Self::Sgl(f) => f.config().radio_model.to_owned(),
            Self::Cs(_) => cs::RADIO_MODEL.to_owned(),
            Self::Ailunce(_) => ailunce::RADIO_MODEL.to_owned(),
            Self::Yaesu(_) => "FT70".to_owned(),
        }
    }

    /// The regions this firmware is written to
    pub fn segments(&self) -> Vec<Segment<'_>> {
        match self {
            Self::Tyt(f) => f.segments(),
            Self::Sgl(f) => f.segments(),
            Self::Cs(f) => f.segments(),
            Self::Ailunce(f) => f.segments(),
            Self::Yaesu(f) => f.segments(),
        }
    }

    /// The bytes to send to a radio, which are the bytes as stored in the
    /// file rather than the deciphered image.
    ///
    /// A radio's bootloader takes the stored form and deciphers it itself.
    /// Sending the deciphered image gives a radio that accepts every block,
    /// reports success, and then will not start.
    pub fn segments_as_stored(&self) -> Result<Vec<(u32, Vec<u8>)>, String> {
        match self {
            Self::Tyt(f) => Ok(f.segments_as_stored()),
            Self::Sgl(f) => Ok(vec![(0, f.data_as_stored())]),
            other => Err(format!(
                "writing {} firmware to a radio is not implemented yet",
                other.radio()
            )),
        }
    }

    /// What an SGL radio needs to be told about itself before it will take
    /// firmware, all of it from the file's header
    pub fn sgl_identity(&self) -> Option<device::hid::flashing::Identity> {
        let Self::Sgl(f) = self else {
            return None;
        };
        let config = f.config();
        Some(device::hid::flashing::Identity {
            model_key: f.model_key()?,
            radio_group: config.radio_group.as_bytes().to_vec(),
            radio_model: config.header_model.as_bytes().to_vec(),
            protocol_version: config.protocol_version.as_bytes().to_vec(),
        })
    }

    /// A human readable summary
    pub fn describe(&self) -> String {
        let mut out = String::new();
        let kind = match self {
            Self::Tyt(_) => "TYT",
            Self::Sgl(_) => "TYT SGL",
            Self::Cs(_) => "Connect Systems",
            Self::Ailunce(_) => "Ailunce",
            Self::Yaesu(_) => "Yaesu",
        };
        let _ = writeln!(out, "== {kind} Firmware ==");
        let _ = writeln!(out, "Radio: {}", self.radio());

        if let Self::Sgl(f) = self {
            let _ = writeln!(out, "Group: {}", f.config().radio_group);
            let _ = writeln!(out, "Model: {}", f.config().header_model);
            let _ = writeln!(out, "Protocol Version: {}", f.config().protocol_version);
            if let Some(key) = f.model_key() {
                // the key that opens a flashing session, four bytes of prefix
                // and four the writer chose
                let _ = writeln!(out, "Key: {}", String::from_utf8_lossy(&key));
            }
            if let Some(s) = f.secrets() {
                let _ = writeln!(out, "Binary offset: 0x400 + {:#04x}", s.binary_offset);
            }
        }
        if let Self::Cs(f) = self {
            let _ = writeln!(
                out,
                "Checksum: {}",
                if f.had_checksum() {
                    "present and correct"
                } else {
                    "absent, as CSFWTOOL writes them"
                }
            );
        }
        if let Self::Ailunce(f) = self {
            let lossy = f.lossy_offsets();
            if !lossy.is_empty() {
                let _ = writeln!(
                    out,
                    "Warning: {} bytes cannot survive this radio's obfuscation, first at {:#x}",
                    lossy.len(),
                    lossy.first().copied().unwrap_or(0)
                );
            }
        }

        let total: usize = self.segments().iter().map(|s| s.data.len()).sum();
        let _ = writeln!(out, "Size:  {}", format_bytes(total as u64));
        let _ = writeln!(out, "Data Segments:");
        for (ix, seg) in self.segments().iter().enumerate() {
            let _ = writeln!(
                out,
                "  {ix}: Start={:#010x}, Length={:#010x}",
                seg.address,
                seg.data.len()
            );
        }
        out
    }
}

/// Bytes in the largest unit that leaves a number above one
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [(u64, &str); 5] = [
        (1 << 50, "PiB"),
        (1 << 40, "TiB"),
        (1 << 30, "GiB"),
        (1 << 20, "MiB"),
        (1 << 10, "kiB"),
    ];
    for (size, name) in UNITS {
        if bytes >= size {
            return format!("{:.2} {name}", bytes as f64 / size as f64);
        }
    }
    format!("{bytes} B")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn byte_counts_use_every_unit() {
        // the C++ skipped MiB entirely, so a 4 MB firmware read as 4096 kiB
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.00 kiB");
        assert_eq!(format_bytes(4 * 1024 * 1024), "4.00 MiB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.00 GiB");
    }

    #[test]
    fn nothing_is_identified_as_firmware_by_accident() {
        assert!(Firmware::identify(&[]).is_none());
        assert!(Firmware::identify(&[0u8; 512]).is_none());
        assert!(Firmware::identify(b"not firmware at all").is_none());
    }
}
