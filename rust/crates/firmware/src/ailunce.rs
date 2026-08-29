//! The Ailunce HD1 firmware container.
//!
//! There is no container: the file is the firmware, obfuscated a word at a
//! time. Nothing in the file identifies the radio, so a file can only be
//! recognised by the user naming the model.
//!
//! # The obfuscation is not reversible for every input
//!
//! The C++ uses one function for both directions, on the assumption that the
//! transform is its own inverse. That holds for almost every whole word, and
//! for hardly any trailing byte.
//!
//! Whole words pick their branch on bit 28, which neither mask touches, so
//! the branch is the same on the way back. Two words break it anyway, by
//! landing on the values the first branch claims:
//!
//! | word | encrypts to | decrypts back to |
//! | - | - | - |
//! | `0x00000000` | `0xffffffff` | `0x00000000` |
//! | `0xffffffff` | `0x00000000` | `0xffffffff` |
//! | `0x07777777` | `0x00000000` | `0xffffffff` (wrong) |
//! | `0xfeeeeeee` | `0xffffffff` | `0x00000000` (wrong) |
//!
//! The trailing bytes of a firmware whose length is not a multiple of four
//! are worse. They pick their branch on bit 0, which both of their masks
//! flip, so the second pass takes the other branch and lands somewhere else
//! entirely. Every value except `0x00` and `0xff` is affected: `0x44`
//! encrypts to `0x43` and decrypts back to `0x42`. It looks like the word
//! rule transcribed to bytes without noticing that bit 28 was load bearing.
//!
//! Both were verified against the C++ by wrapping a file twice and comparing.
//! Real firmware is word aligned and mostly padding, which is why this has
//! never been noticed.
//!
//! This port keeps the behaviour, because the radio defines the format and
//! writing something else would produce firmware it cannot decode. Use
//! [`lossy_offsets`] to find out whether a particular binary is affected.

use crate::{Error, Result, Segment};

/// The only radio this container is known to belong to
pub const RADIO_MODEL: &str = "HD1";

/// Is this a radio this container supports
pub fn supports_model(model: &str) -> bool {
    model == RADIO_MODEL
}

/// Obfuscate, or de-obfuscate, in place.
///
/// See the module docs: this is its own inverse for every word except
/// `0x07777777` and `0xfeeeeeee`.
pub fn transform(data: &mut [u8]) {
    let mut chunks = data.chunks_exact_mut(4);
    for word in chunks.by_ref() {
        let Ok(bytes) = <[u8; 4]>::try_from(&word[..]) else {
            continue;
        };
        let value = u32::from_le_bytes(bytes);

        let out = if value == 0x0000_0000 || value == 0xffff_ffff {
            value ^ 0xffff_ffff
        } else if value & (1 << 28) != 0 {
            value ^ 0x0111_1111
        } else {
            value ^ 0x0777_7777
        };

        word.copy_from_slice(&out.to_le_bytes());
    }

    // whatever does not make up a whole word is handled a byte at a time,
    // with the same shape of rule
    for byte in chunks.into_remainder() {
        *byte = if *byte == 0x00 || *byte == 0xff {
            *byte ^ 0xff
        } else if *byte & 1 != 0 {
            *byte ^ 0x01
        } else {
            *byte ^ 0x07
        };
    }
}

/// Byte offsets that will not survive a round trip.
///
/// A firmware binary holding any of these will be decoded by the radio as
/// something other than what went in. Padding, which is to say `0x00000000`
/// and `0xffffffff` words and `0x00` and `0xff` bytes, is safe and is not
/// reported. See the module docs for why.
pub fn lossy_offsets(data: &[u8]) -> Vec<usize> {
    let whole = data.len() - (data.len() % 4);

    let words = data.chunks_exact(4).enumerate().filter_map(|(ix, word)| {
        let bytes = <[u8; 4]>::try_from(word).ok()?;
        let value = u32::from_le_bytes(bytes);
        (value == 0x0777_7777 || value == 0xfeee_eeee).then_some(ix * 4)
    });

    // every trailing byte other than the two padding values is affected
    let tail = data
        .get(whole..)
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter_map(move |(ix, byte)| (*byte != 0x00 && *byte != 0xff).then_some(whole + ix));

    words.chain(tail).collect()
}

/// An Ailunce firmware file, holding plain firmware data
#[derive(Debug, Clone, Default)]
pub struct AilunceFirmware {
    data: Vec<u8>,
}

impl AilunceFirmware {
    /// Start a new firmware file for a radio
    pub fn new(model: &str) -> Result<Self> {
        if !supports_model(model) {
            return Err(Error::UnknownModel(model.to_owned()));
        }
        Ok(Self::default())
    }

    /// Plain firmware data
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

    /// Set the firmware data
    pub fn set_data(&mut self, data: &[u8]) -> Result<()> {
        self.data = data.to_vec();
        Ok(())
    }

    /// Byte offsets in this firmware the radio will decode incorrectly
    pub fn lossy_offsets(&self) -> Vec<usize> {
        lossy_offsets(&self.data)
    }

    /// Any Ailunce firmware is compatible with any other, nothing in the file
    /// says otherwise
    pub fn is_compatible(&self, _other: &Self) -> bool {
        true
    }

    /// Read a firmware file. Every byte is firmware, so this cannot fail on
    /// structure, only produce nonsense for a file that is not Ailunce
    /// firmware in the first place.
    pub fn parse(input: &[u8]) -> Result<Self> {
        let mut data = input.to_vec();
        transform(&mut data);
        Ok(Self { data })
    }

    /// Write a firmware file
    pub fn serialise(&self) -> Result<Vec<u8>> {
        if self.data.is_empty() {
            return Err(Error::NoSegments);
        }
        let mut out = self.data.clone();
        transform(&mut out);
        Ok(out)
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

    fn words(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn as_words(data: &[u8]) -> Vec<u32> {
        data.chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().expect("four bytes")))
            .collect()
    }

    #[test]
    fn matches_the_values_observed_from_the_cpp() {
        // taken from wrapping a file of these words with the C++ tool
        let cases = [
            (0x0000_0000u32, 0xffff_ffffu32),
            (0xffff_ffff, 0x0000_0000),
            (0x0777_7777, 0x0000_0000),
            (0xfeee_eeee, 0xffff_ffff),
            (0x0111_1111, 0x0666_6666),
            (0x1234_5678, 0x1325_4769),
            (0x1000_0000, 0x1111_1111),
        ];

        for (input, want) in cases {
            let mut data = words(&[input]);
            transform(&mut data);
            assert_eq!(
                as_words(&data)[0],
                want,
                "{input:#010x} should encrypt to {want:#010x}"
            );
        }
    }

    #[test]
    fn is_its_own_inverse_for_ordinary_words() {
        let plain = words(&[0x1234_5678, 0x0111_1111, 0x1000_0000, 0xdead_beef, 0]);
        let mut data = plain.clone();
        transform(&mut data);
        assert_ne!(data, plain);
        transform(&mut data);
        assert_eq!(data, plain);
    }

    #[test]
    fn is_not_its_own_inverse_for_the_two_known_words() {
        // this documents a flaw in the format, not in this port. If it ever
        // starts passing, the format changed and the radios did too.
        for value in [0x0777_7777u32, 0xfeee_eeee] {
            let plain = words(&[value]);
            let mut data = plain.clone();
            transform(&mut data);
            transform(&mut data);
            assert_ne!(
                data, plain,
                "{value:#010x} unexpectedly round trips, has the format changed?"
            );
        }
    }

    #[test]
    fn lossy_offsets_finds_the_two_words_and_skips_padding() {
        let data = words(&[
            0x1234_5678,
            0x0777_7777,
            0x0000_0000,
            0xffff_ffff,
            0xfeee_eeee,
        ]);

        assert_eq!(
            lossy_offsets(&data),
            vec![4, 16],
            "padding is safe and must not be reported"
        );
        assert!(lossy_offsets(&words(&[0x1234_5678, 0, u32::MAX])).is_empty());
    }

    #[test]
    fn lossy_offsets_reports_the_tail() {
        // one whole word, then three bytes that cannot survive
        let mut data = words(&[0x1234_5678]);
        data.extend_from_slice(&[0x44, 0x00, 0x46]);

        assert_eq!(
            lossy_offsets(&data),
            vec![4, 6],
            "0x44 and 0x46 cannot survive, the 0x00 between them can"
        );
    }

    #[test]
    fn the_tail_is_transformed_but_does_not_round_trip() {
        // this documents a flaw in the format, not in this port. The byte
        // rule picks its branch on bit 0 and then flips it, so the second
        // pass takes the other branch. Verified against the C++.
        let mut data = vec![0x44u8, 0x46];
        transform(&mut data);
        assert_eq!(data, vec![0x43, 0x41], "must match the C++ byte for byte");
        transform(&mut data);
        assert_eq!(data, vec![0x42, 0x40], "the tail lands two away, not back");

        // padding is the only tail that survives
        let mut padding = vec![0x00u8, 0xff];
        transform(&mut padding);
        assert_eq!(padding, vec![0xff, 0x00]);
        transform(&mut padding);
        assert_eq!(padding, vec![0x00, 0xff], "padding round trips");
    }

    #[test]
    fn a_word_aligned_file_round_trips() {
        // which is what real firmware is, and why none of this has bitten
        for words_in in 1..8usize {
            let plain: Vec<u8> = (0..words_in * 4).map(|x| (x as u8) | 0x40).collect();
            let mut data = plain.clone();
            transform(&mut data);
            assert_ne!(data, plain, "{words_in} words were left alone");
            transform(&mut data);
            assert_eq!(data, plain, "{words_in} words did not round trip");
        }
    }

    #[test]
    fn round_trip_through_a_file() {
        let plain: Vec<u8> = (0..0x400).map(|x| (x % 253) as u8).collect();

        let mut fw = AilunceFirmware::new(RADIO_MODEL).expect("model is supported");
        fw.set_data(&plain).expect("data fits");
        let file = fw.serialise().expect("serialises");

        assert_eq!(file.len(), plain.len(), "this container adds nothing");
        assert_ne!(file, plain, "the firmware was written in the clear");

        let back = AilunceFirmware::parse(&file).expect("parses");
        assert_eq!(back.data(), plain);
        assert_eq!(back.segments().len(), 1);
        assert_eq!(back.segments()[0].address, 0);
    }

    #[test]
    fn unknown_model_is_rejected() {
        assert!(matches!(
            AilunceFirmware::new("NOT-A-RADIO"),
            Err(Error::UnknownModel(_))
        ));
        assert!(supports_model("HD1"));
    }

    #[test]
    fn empty_firmware_will_not_serialise() {
        let fw = AilunceFirmware::new(RADIO_MODEL).expect("model is supported");
        assert!(matches!(fw.serialise(), Err(Error::NoSegments)));
    }

    #[test]
    fn any_length_of_input_is_handled() {
        for len in 0..0x40 {
            let data: Vec<u8> = (0..len).map(|x| x as u8).collect();
            let parsed = AilunceFirmware::parse(&data).expect("parses");
            assert_eq!(parsed.data().len(), len);
        }
    }
}
