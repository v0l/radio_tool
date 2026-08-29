//! The Yaesu FT-70D firmware container.
//!
//! There is no container and no obfuscation: the file is the firmware. The
//! only handling it needs is padding to a whole number of kibibytes, because
//! the radio is written a 1 KiB block at a time.
//!
//! Nothing in the file identifies the radio, so a file can only be recognised
//! by the user naming the model.

use crate::{Error, Result, Segment};

/// The radio is written in blocks of this size
pub const BLOCK_LEN: usize = 1024;

/// Names this container answers to
pub const RADIO_MODELS: &[&str] = &["FT70", "FT70D", "FT-70D"];

/// Is this a radio this container supports
pub fn supports_model(model: &str) -> bool {
    RADIO_MODELS.contains(&model)
}

/// A Yaesu firmware file
#[derive(Debug, Clone, Default)]
pub struct YaesuFirmware {
    data: Vec<u8>,
}

impl YaesuFirmware {
    /// Start a new firmware file for a radio
    pub fn new(model: &str) -> Result<Self> {
        if !supports_model(model) {
            return Err(Error::UnknownModel(model.to_owned()));
        }
        Ok(Self::default())
    }

    /// Firmware data, padded out to a whole number of blocks
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// This container holds a single region, written from the start
    pub fn segments(&self) -> Vec<Segment<'_>> {
        vec![Segment {
            address: 0,
            data: &self.data,
        }]
    }

    /// Set the firmware data, padding it to a whole number of blocks
    pub fn set_data(&mut self, data: &[u8]) -> Result<()> {
        self.data = data.to_vec();
        pad_to_block(&mut self.data);
        Ok(())
    }

    /// Any Yaesu firmware is compatible with any other, nothing in the file
    /// says otherwise
    pub fn is_compatible(&self, _other: &Self) -> bool {
        true
    }

    /// Read a firmware file, padding it to a whole number of blocks
    pub fn parse(input: &[u8]) -> Result<Self> {
        let mut data = input.to_vec();
        pad_to_block(&mut data);
        Ok(Self { data })
    }

    /// Write a firmware file
    pub fn serialise(&self) -> Result<Vec<u8>> {
        if self.data.is_empty() {
            return Err(Error::NoSegments);
        }
        Ok(self.data.clone())
    }
}

/// Pad with 0xff up to a whole number of blocks
fn pad_to_block(data: &mut Vec<u8>) {
    let padded = data.len().next_multiple_of(BLOCK_LEN);
    data.resize(padded, 0xff);
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
    fn short_firmware_is_padded_to_a_block() {
        let fw = YaesuFirmware::parse(&[1, 2, 3]).expect("parses");
        assert_eq!(fw.data().len(), BLOCK_LEN);
        assert_eq!(&fw.data()[..3], &[1, 2, 3]);
        assert!(fw.data()[3..].iter().all(|b| *b == 0xff), "padding is 0xff");
    }

    #[test]
    fn a_whole_number_of_blocks_is_left_alone() {
        for blocks in 1..4usize {
            let data = vec![0x5a; blocks * BLOCK_LEN];
            let fw = YaesuFirmware::parse(&data).expect("parses");
            assert_eq!(fw.data(), data, "{blocks} blocks were changed");
        }
    }

    #[test]
    fn one_byte_over_a_block_pads_to_two() {
        let data = vec![0x5a; BLOCK_LEN + 1];
        let fw = YaesuFirmware::parse(&data).expect("parses");
        assert_eq!(fw.data().len(), BLOCK_LEN * 2);
    }

    #[test]
    fn empty_stays_empty_and_will_not_serialise() {
        let fw = YaesuFirmware::parse(&[]).expect("parses");
        assert!(fw.data().is_empty(), "nothing to pad");
        assert!(matches!(fw.serialise(), Err(Error::NoSegments)));
    }

    #[test]
    fn the_firmware_is_written_out_untouched() {
        let data: Vec<u8> = (0..BLOCK_LEN).map(|x| x as u8).collect();
        let mut fw = YaesuFirmware::new("FT70").expect("model is supported");
        fw.set_data(&data).expect("data fits");

        // no header, no cipher, so the file is the firmware
        assert_eq!(fw.serialise().expect("serialises"), data);
    }

    #[test]
    fn model_names_are_accepted_as_the_cpp_does() {
        for model in RADIO_MODELS {
            assert!(YaesuFirmware::new(model).is_ok(), "{model} was rejected");
        }
        // this used to accept every model, which stole HD1 from the Ailunce
        // handler and wrote its firmware out unencrypted
        assert!(!supports_model("HD1"));
        assert!(!supports_model("DM1701"));
        assert!(matches!(
            YaesuFirmware::new("HD1"),
            Err(Error::UnknownModel(_))
        ));
    }
}
