//! XOR cipher tables used by the firmware containers.
//!
//! The tables are byte for byte copies of the arrays in
//! `include/radio_tool/fw/cipher/*.hpp`, extracted mechanically by
//! `rust/tools/extract_ciphers.py`. They are never edited by hand.

/// TYT DM-1701
pub const DM1701: &[u8] = include_bytes!("../ciphers/dm1701.bin");
/// TYT MD-380 / MD-390 / MD-446 / MD-280
pub const MD380: &[u8] = include_bytes!("../ciphers/md380.bin");
/// TYT MD-9600
pub const MD9600: &[u8] = include_bytes!("../ciphers/md9600.bin");
/// TYT MD-UV380 / MD-UV390 / MD-2017
pub const UV3X0: &[u8] = include_bytes!("../ciphers/uv3x0.bin");
/// Radioddity GD-77 and friends (SGL container)
pub const SGL: &[u8] = include_bytes!("../ciphers/sgl.bin");
/// Connect Systems CS800
pub const CS800_0: &[u8] = include_bytes!("../ciphers/cs800_0.bin");
/// Connect Systems CS800, second table
pub const CS800_1: &[u8] = include_bytes!("../ciphers/cs800_1.bin");
/// Retevis DR-5xx0
pub const DR5XX0: &[u8] = include_bytes!("../ciphers/dr5xx0.bin");

/// XOR `data` against `key`, repeating the key and starting `offset` bytes
/// into it. Symmetric, so the same call encrypts and decrypts.
///
/// # Panics
/// Never. An empty key leaves the data untouched.
pub fn apply_xor(data: &mut [u8], key: &[u8], offset: usize) {
    if key.is_empty() {
        return;
    }
    // walking a cycled iterator keeps this free of indexing, so there is no
    // bounds check to get wrong and nothing that can panic
    let keystream = key.iter().cycle().skip(offset % key.len());
    for (byte, k) in data.iter_mut().zip(keystream) {
        *byte ^= *k;
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
    fn tables_have_the_lengths_the_cpp_headers_declare() {
        assert_eq!(DM1701.len(), 1024);
        assert_eq!(MD380.len(), 1024);
        assert_eq!(MD9600.len(), 1024);
        assert_eq!(UV3X0.len(), 1024);
        assert_eq!(SGL.len(), 32768);
        assert_eq!(CS800_0.len(), 256);
        assert_eq!(CS800_1.len(), 256);
        assert_eq!(DR5XX0.len(), 256);
    }

    #[test]
    fn tables_are_not_all_the_same() {
        // a copy/paste slip in the extractor would show up here
        assert_ne!(DM1701, MD380);
        assert_ne!(MD380, MD9600);
        assert_ne!(MD9600, UV3X0);
        assert_ne!(CS800_0, CS800_1);
    }

    #[test]
    fn xor_is_symmetric() {
        let plain: Vec<u8> = (0u32..1000).map(|x| (x % 251) as u8).collect();

        for offset in [0usize, 1, 512, 1023, 4096] {
            let mut data = plain.clone();
            apply_xor(&mut data, DM1701, offset);
            assert_ne!(data, plain, "offset {offset} left the data unchanged");
            apply_xor(&mut data, DM1701, offset);
            assert_eq!(data, plain, "offset {offset} did not round trip");
        }
    }

    #[test]
    fn xor_matches_a_hand_worked_example() {
        let mut data = [0x00, 0xff, 0x55];
        apply_xor(&mut data, &[0x0f, 0xf0], 0);
        assert_eq!(data, [0x0f, 0x0f, 0x5a]);

        // starting one byte into the key shifts which key byte each input sees
        let mut data = [0x00, 0xff, 0x55];
        apply_xor(&mut data, &[0x0f, 0xf0], 1);
        assert_eq!(data, [0xf0, 0xf0, 0xa5]);
    }

    #[test]
    fn empty_key_is_a_no_op() {
        let mut data = [1, 2, 3];
        apply_xor(&mut data, &[], 0);
        assert_eq!(data, [1, 2, 3]);
    }
}
