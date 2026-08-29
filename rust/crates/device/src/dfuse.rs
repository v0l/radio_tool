//! ST's DfuSe memory layout strings.
//!
//! An ST DFU device describes its own memory in the interface string of each
//! alternate setting, in a format documented in ST's AN3156:
//!
//! ```text
//! @Internal Flash   /0x0800C000/01*016Kg,01*064Kg,07*128Kg
//! @SPI Flash Memory /0x00000000/16*064Kg
//! ```
//!
//! A name, then one or more groups of a start address and a list of sector
//! runs, each `count*size` with a unit and a letter giving what may be done
//! to it.
//!
//! Reading this rather than hardcoding a chip layout is a safety measure, not
//! a convenience. The two strings above came off a DM-1701, and its internal
//! flash begins at `0x0800C000`: the radio is deliberately not offering the
//! three 16K sectors below that, because its bootloader is in them. A
//! hardcoded STM32F40x map covers the whole chip from `0x08000000`, so
//! trusting one would permit erasing the bootloader on a radio that was
//! trying to tell us not to.

use crate::flash::Sector;
use crate::{Error, Result};

/// What may be done to a region
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    /// May be read
    pub readable: bool,
    /// May be erased
    pub erasable: bool,
    /// May be written
    pub writable: bool,
}

impl Permissions {
    /// Decode the letter ending a sector run.
    ///
    /// The letter is `a` plus a three bit mask: 1 readable, 2 erasable,
    /// 4 writable. Some devices use upper case for the same thing.
    fn from_letter(letter: char) -> Option<Self> {
        let bits = match letter {
            'a'..='g' => (letter as u8) - b'a' + 1,
            'A'..='G' => (letter as u8) - b'A' + 1,
            _ => return None,
        };

        Some(Self {
            readable: bits & 0b001 != 0,
            erasable: bits & 0b010 != 0,
            writable: bits & 0b100 != 0,
        })
    }

    /// Can firmware be put here
    pub fn can_program(&self) -> bool {
        self.erasable && self.writable
    }
}

/// One memory region a device offers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    /// What the device calls it, such as `Internal Flash`
    pub name: String,
    /// Which alternate setting selects it
    pub alt: u8,
    /// Its sectors, in address order
    pub sectors: Vec<Sector>,
    /// What may be done to each sector, in the same order
    pub permissions: Vec<Permissions>,
}

impl Memory {
    /// First address of the region
    pub fn start(&self) -> Option<u32> {
        self.sectors.first().map(|s| s.start)
    }

    /// First address past the region
    pub fn end(&self) -> Option<u32> {
        self.sectors.last().map(Sector::end)
    }

    /// Total size
    pub fn size(&self) -> u64 {
        self.sectors.iter().map(|s| u64::from(s.size)).sum()
    }

    /// The sectors firmware may be written to
    pub fn programmable(&self) -> Vec<Sector> {
        self.sectors
            .iter()
            .zip(&self.permissions)
            .filter(|(_, p)| p.can_program())
            .map(|(s, _)| *s)
            .collect()
    }
}

impl std::fmt::Display for Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "alt {}: {} at {:#010x}, {} in {} sectors",
            self.alt,
            self.name,
            self.start().unwrap_or(0),
            crate::dfuse::human_size(self.size()),
            self.sectors.len()
        )
    }
}

/// Sizes the way the layout string writes them
fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 && bytes % (1024 * 1024) == 0 {
        format!("{} MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 && bytes % 1024 == 0 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} bytes")
    }
}

/// Read one alternate setting's layout string
pub fn parse(text: &str, alt: u8) -> Result<Memory> {
    let text = text.trim();

    let body = text.strip_prefix('@').ok_or(Error::Unexpected {
        what: "memory layout",
        expected: "a string beginning with @".to_owned(),
        got: text.chars().take(32).collect(),
    })?;

    let mut parts = body.split('/');
    let name = parts
        .next()
        .ok_or(Error::Unexpected {
            what: "memory layout",
            expected: "a name".to_owned(),
            got: "nothing".to_owned(),
        })?
        .trim()
        .to_owned();

    let mut sectors = Vec::new();
    let mut permissions = Vec::new();
    let mut index: u8 = 0;

    // the rest is pairs of an address and the runs that start there
    loop {
        let Some(address) = parts.next() else {
            break;
        };
        let runs = parts.next().ok_or(Error::Unexpected {
            what: "memory layout",
            expected: "sector sizes after an address".to_owned(),
            got: format!("{address} and nothing more"),
        })?;

        let address = address.trim();
        let mut at = u32::from_str_radix(address.strip_prefix("0x").unwrap_or(address).trim(), 16)
            .map_err(|_| Error::Unexpected {
                what: "memory layout",
                expected: "a hexadecimal start address".to_owned(),
                got: address.to_owned(),
            })?;

        for run in runs.split(',') {
            let (count, size, permission) = parse_run(run.trim())?;

            for _ in 0..count {
                sectors.push(Sector {
                    index,
                    start: at,
                    size,
                });
                permissions.push(permission);
                index = index.wrapping_add(1);
                at = at.checked_add(size).ok_or(Error::Unexpected {
                    what: "memory layout",
                    expected: "sectors inside the address space".to_owned(),
                    got: "a layout that runs off the end of it".to_owned(),
                })?;
            }
        }
    }

    if sectors.is_empty() {
        return Err(Error::Unexpected {
            what: "memory layout",
            expected: "at least one sector".to_owned(),
            got: text.to_owned(),
        });
    }

    Ok(Memory {
        name,
        alt,
        sectors,
        permissions,
    })
}

/// One `count*size` run, such as `07*128Kg`
fn parse_run(run: &str) -> Result<(u32, u32, Permissions)> {
    let bad = || Error::Unexpected {
        what: "sector run",
        expected: "something like 07*128Kg".to_owned(),
        got: run.to_owned(),
    };

    let (count, rest) = run.split_once('*').ok_or_else(bad)?;
    let count: u32 = count.trim().parse().map_err(|_| bad())?;

    let mut chars = rest.trim().chars().collect::<Vec<_>>();
    let letter = chars.pop().ok_or_else(bad)?;
    let permission = Permissions::from_letter(letter).ok_or_else(bad)?;

    // the unit is optional, and a space means bytes
    let (digits, multiplier) = match chars.last() {
        Some('K' | 'k') => {
            chars.pop();
            (chars.iter().collect::<String>(), 1024u32)
        }
        Some('M' | 'm') => {
            chars.pop();
            (chars.iter().collect::<String>(), 1024 * 1024)
        }
        Some('B' | 'b' | ' ') => {
            chars.pop();
            (chars.iter().collect::<String>(), 1)
        }
        _ => (chars.iter().collect::<String>(), 1),
    };

    let size: u32 = digits.trim().parse().map_err(|_| bad())?;
    let size = size.checked_mul(multiplier).ok_or_else(bad)?;

    if count == 0 || size == 0 {
        return Err(bad());
    }

    Ok((count, size, permission))
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

    /// Exactly what a DM-1701 reports, read off the radio over USB
    const DM1701_INTERNAL: &str = "@Internal Flash   /0x0800C000/01*016Kg,01*064Kg,07*128Kg";
    const DM1701_SPI: &str = "@SPI Flash Memory /0x00000000/16*064Kg";

    #[test]
    fn a_real_radio_layout_is_read_correctly() {
        let mem = parse(DM1701_INTERNAL, 0).expect("parses");

        assert_eq!(mem.name, "Internal Flash");
        assert_eq!(mem.sectors.len(), 1 + 1 + 7);
        assert_eq!(mem.start(), Some(0x0800_C000));

        // 16K, then 64K, then seven of 128K
        assert_eq!(mem.sectors[0].size, 16 * 1024);
        assert_eq!(mem.sectors[1].size, 64 * 1024);
        assert!(mem.sectors[2..].iter().all(|s| s.size == 128 * 1024));

        // and they follow one another with no gaps
        for pair in mem.sectors.windows(2) {
            assert_eq!(pair[1].start, pair[0].end());
        }
        assert_eq!(mem.end(), Some(0x0810_0000), "it reaches the end of flash");
        assert_eq!(mem.size(), 0x0f_4000);
    }

    /// The point of reading this from the device rather than hardcoding a
    /// chip layout: the radio withholds the sectors its bootloader is in
    #[test]
    fn the_radio_does_not_offer_the_sectors_its_bootloader_is_in() {
        let mem = parse(DM1701_INTERNAL, 0).expect("parses");

        assert_eq!(
            mem.start(),
            Some(0x0800_C000),
            "an STM32F40x starts at 0x08000000, and this radio does not offer that"
        );
        assert!(
            !mem.sectors.iter().any(|s| s.start < 0x0800_C000),
            "no sector below the bootloader may be offered"
        );

        // the hardcoded chip map covers what the radio holds back
        let chip_start = crate::flash::STM32F40X.first().expect("a map").start;
        assert!(
            chip_start < mem.start().expect("a start"),
            "the chip map reaches lower than the radio permits, which is the hazard"
        );
    }

    #[test]
    fn the_spi_flash_is_read_correctly() {
        let mem = parse(DM1701_SPI, 1).expect("parses");
        assert_eq!(mem.name, "SPI Flash Memory");
        assert_eq!(mem.alt, 1);
        assert_eq!(mem.sectors.len(), 16);
        assert_eq!(mem.start(), Some(0));
        assert_eq!(mem.size(), 1024 * 1024);
    }

    #[test]
    fn permissions_are_decoded_from_the_letter() {
        let all = Permissions::from_letter('g').expect("g is valid");
        assert!(all.readable && all.erasable && all.writable);
        assert!(all.can_program());

        let readonly = Permissions::from_letter('a').expect("a is valid");
        assert!(readonly.readable);
        assert!(!readonly.erasable && !readonly.writable);
        assert!(
            !readonly.can_program(),
            "read only memory is not programmable"
        );

        // upper case means the same thing
        assert_eq!(Permissions::from_letter('G'), Permissions::from_letter('g'));

        assert!(Permissions::from_letter('h').is_none());
        assert!(Permissions::from_letter('1').is_none());
    }

    #[test]
    fn read_only_sectors_are_not_offered_for_programming() {
        // option bytes, which are readable and erasable but not writable
        let mem = parse("@Option Bytes /0x1FFFC000/01*016 e", 2).expect("parses");
        assert_eq!(mem.sectors.len(), 1);
        assert_eq!(mem.sectors[0].size, 16);

        let mem = parse("@Mixed /0x08000000/02*016Ka,02*016Kg", 0).expect("parses");
        assert_eq!(mem.sectors.len(), 4);
        assert_eq!(
            mem.programmable().len(),
            2,
            "only the sectors that allow it may be programmed"
        );
    }

    #[test]
    fn units_are_understood() {
        for (text, expected) in [
            ("@x /0/01*016Kg", 16 * 1024),
            ("@x /0/01*016kg", 16 * 1024),
            ("@x /0/01*002Mg", 2 * 1024 * 1024),
            ("@x /0/01*256 g", 256),
            ("@x /0/01*256Bg", 256),
            ("@x /0/01*256g", 256),
        ] {
            let mem = parse(text, 0).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(mem.sectors[0].size, expected, "{text}");
        }
    }

    #[test]
    fn several_address_groups_in_one_string_are_all_read() {
        let mem = parse("@Flash /0x08000000/02*016Kg/0x08100000/01*064Kg", 0).expect("parses");
        assert_eq!(mem.sectors.len(), 3);
        assert_eq!(mem.sectors[0].start, 0x0800_0000);
        assert_eq!(mem.sectors[2].start, 0x0810_0000);
    }

    #[test]
    fn rubbish_is_refused_rather_than_guessed_at() {
        for bad in [
            "",
            "Internal Flash /0x08000000/01*016Kg", // no @
            "@Flash",                              // no address
            "@Flash /0x08000000",                  // an address and no sectors
            "@Flash /notanaddress/01*016Kg",
            "@Flash /0x08000000/01*016Kz", // not a permission letter
            "@Flash /0x08000000/016Kg",    // no count
            "@Flash /0x08000000/00*016Kg", // no sectors in the run
            "@Flash /0x08000000/01*000Kg", // sectors of nothing
            "@Flash /0x08000000/ab*016Kg",
        ] {
            assert!(parse(bad, 0).is_err(), "{bad:?} should not have parsed");
        }
    }

    #[test]
    fn a_layout_that_would_run_off_the_end_of_memory_is_refused() {
        assert!(parse("@Flash /0xFFFF0000/16*064Kg", 0).is_err());
    }

    #[test]
    fn arbitrary_text_never_panics() {
        let sample = DM1701_INTERNAL;
        for len in 0..sample.len() {
            let _ = parse(&sample[..len], 0);
        }
        for junk in [
            "@@@@",
            "@/",
            "@//",
            "@a/0/*",
            "@a/0/1*",
            "@\u{1f600}/0/01*016Kg",
        ] {
            let _ = parse(junk, 0);
        }
    }
}
