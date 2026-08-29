//! Guessing the XOR key of an unknown firmware.
//!
//! These radios encrypt firmware with a repeating XOR key, which is no
//! encryption at all: compiled firmware is full of zero bytes, so the most
//! common byte at each position of the key is almost always the key byte
//! itself. That is enough to recover the key of a radio nobody has looked at
//! before, which is how the keys in [`crate::cipher`] were found.
//!
//! Ported from the C++ `XORTool`, with the counter overflow fixed: it counted
//! in a `uint8_t`, so any byte value occurring 256 times wrapped to zero and
//! the guess was wrong for anything but a small file.
//!
//! # How well it actually works
//!
//! Measured against 14 real vendor firmware files, the guess recovers between
//! 60% and 100% of the key, median 65%. The spread is not noise: it tracks
//! how much low entropy data the image holds.
//!
//! | firmware | key recovered |
//! | - | - |
//! | MD-UV380 and UV390, two regions, one of them resources | 99.6% to 100% |
//! | MD-280 | 94% |
//! | MD-380, MD-390 and MD-446, code only | 60% to 67% |
//!
//! Compiled ARM firmware is only about 11% zero bytes, and instruction
//! alignment means each key position sees its own skewed distribution, so a
//! position can easily be won by something other than the key. Images with a
//! resource region, or one mostly padding, do far better.
//!
//! So treat the result as a strong lead rather than an answer: recover a key,
//! decrypt with it, and check the result with [`looks_like_vector_table`].
//! Even the worst case here is over a hundred times better than chance, which
//! is enough to make the rest fall out by inspection.

/// Keys in this family are all this long
pub const KEY_LEN: usize = 1024;

/// Guess the key of an encrypted firmware image.
///
/// Returns a key of [`KEY_LEN`] bytes. Feed it the encrypted firmware, not a
/// decrypted one. A short input gives a poor guess: there needs to be enough
/// data for each key position to see many bytes.
pub fn guess_key(encrypted: &[u8]) -> Vec<u8> {
    // counts have to be wider than a byte, which is what the C++ got wrong
    let mut histogram = vec![[0u32; 256]; KEY_LEN];

    for (ix, byte) in encrypted.iter().enumerate() {
        if let Some(row) = histogram.get_mut(ix % KEY_LEN) {
            if let Some(count) = row.get_mut(usize::from(*byte)) {
                *count += 1;
            }
        }
    }

    histogram
        .iter()
        .map(|row| {
            let mut best = 0u8;
            let mut best_count = 0u32;
            for (value, count) in row.iter().enumerate() {
                if *count > best_count {
                    best_count = *count;
                    best = value as u8;
                }
            }
            best
        })
        .collect()
}

/// How much of `guess` matches `actual`, as a fraction from 0 to 1
pub fn key_agreement(guess: &[u8], actual: &[u8]) -> f64 {
    if actual.is_empty() {
        return 0.0;
    }
    let same = guess.iter().zip(actual).filter(|(a, b)| a == b).count();
    same as f64 / actual.len() as f64
}

/// Does the start of this decrypted firmware look like an ARM vector table.
///
/// The first word is the initial stack pointer, which lives in SRAM, and the
/// rest are interrupt handler addresses, which have to point inside the
/// firmware. A key that produces a plausible vector table is almost certainly
/// the right key.
pub fn looks_like_vector_table(base_address: u32, decrypted: &[u8]) -> bool {
    const TABLE_ENTRIES: usize = 0x61;

    let Some(table) = decrypted.get(..TABLE_ENTRIES * 4) else {
        return false;
    };
    let word = |ix: usize| -> Option<u32> {
        let b = table.get(ix * 4..(ix * 4) + 4)?;
        Some(u32::from_le_bytes([
            *b.first()?,
            *b.get(1)?,
            *b.get(2)?,
            *b.get(3)?,
        ]))
    };

    // the stack top sits in SRAM, which is 0x2000_0000 on these parts
    match word(0) {
        Some(stack) if (stack & 0x2ffe_0000) == 0x2000_0000 => {}
        _ => return false,
    }

    let end = base_address.saturating_add(decrypted.len() as u32);
    for ix in 1..TABLE_ENTRIES {
        match word(ix) {
            // an unused vector is zero, anything else must point into the image
            Some(0) => continue,
            Some(addr) if addr > base_address && addr < end => continue,
            _ => return false,
        }
    }
    true
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
    use crate::cipher;

    /// Firmware shaped data: mostly zeroes, with some content.
    ///
    /// Which bytes are non zero has to be decided by the generator, not by
    /// the index: the key length divides 4, so `ix % 4` would give some key
    /// positions nothing but content and others nothing but zeroes, and the
    /// guess would be exactly 75% right no matter how good the code was.
    fn firmware_like(len: usize, seed: u32) -> Vec<u8> {
        let mut x = seed | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                // roughly a quarter content, the rest zeroes, which is what
                // compiled firmware padded out to a flash sector looks like
                if x % 4 == 0 { (x >> 8) as u8 } else { 0 }
            })
            .collect()
    }

    #[test]
    fn recovers_a_key_from_encrypted_firmware() {
        let plain = firmware_like(200 * KEY_LEN, 7);
        let mut encrypted = plain.clone();
        cipher::apply_xor(&mut encrypted, cipher::DM1701, 0);

        let guess = guess_key(&encrypted);
        assert_eq!(guess.len(), KEY_LEN);

        let agreement = key_agreement(&guess, cipher::DM1701);
        assert!(
            agreement > 0.99,
            "expected to recover nearly the whole key, got {agreement}"
        );
    }

    #[test]
    fn counts_do_not_overflow_a_byte() {
        // The C++ counted in a uint8_t. Key position zero here sees 0xaa 256
        // times, which wraps that counter to nothing, and 0xbb ten times. A
        // byte counter picks 0xbb, a wider one picks 0xaa.
        let rounds = 266;
        let mut encrypted = vec![0x00u8; rounds * KEY_LEN];
        for round in 0..rounds {
            encrypted[round * KEY_LEN] = if round < 256 { 0xaa } else { 0xbb };
        }

        let guess = guess_key(&encrypted);
        assert_eq!(
            guess[0], 0xaa,
            "0xaa occurs 256 times and 0xbb ten times, so 0xaa wins"
        );
    }

    #[test]
    fn an_empty_or_short_input_still_returns_a_key() {
        assert_eq!(guess_key(&[]).len(), KEY_LEN);
        assert_eq!(guess_key(&[1, 2, 3]).len(), KEY_LEN);
    }

    #[test]
    fn a_real_vector_table_is_recognised() {
        let base = 0x0800_c000u32;
        let mut image = Vec::new();
        image.extend_from_slice(&0x2000_1000u32.to_le_bytes()); // stack in SRAM
        for ix in 1..0x61u32 {
            // handlers point just inside the image
            let addr = if ix % 7 == 0 { 0 } else { base + 0x200 + ix };
            image.extend_from_slice(&addr.to_le_bytes());
        }
        image.resize(0x1000, 0);

        assert!(looks_like_vector_table(base, &image));
    }

    #[test]
    fn nonsense_is_not_a_vector_table() {
        assert!(!looks_like_vector_table(0x0800_c000, &[]));
        assert!(!looks_like_vector_table(0x0800_c000, &[0u8; 0x184]));

        // a plausible stack but handlers pointing nowhere useful
        let mut image = 0x2000_1000u32.to_le_bytes().to_vec();
        image.extend(std::iter::repeat_n(0xab, 0x180));
        assert!(!looks_like_vector_table(0x0800_c000, &image));
    }
}
