//! Flash sector maps, and walking a range of memory a sector at a time.
//!
//! Erasing is per sector, and the sectors are not all the same size, so
//! writing a region means splitting it on sector boundaries. Getting this
//! wrong erases memory that was not being written to, which on a radio means
//! taking out the bootloader or the calibration data next to it.

/// One erasable sector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sector {
    /// Index, as the chip numbers them
    pub index: u8,
    /// Where it starts
    pub start: u32,
    /// How big it is
    pub size: u32,
}

impl Sector {
    /// First address past this sector
    pub fn end(&self) -> u32 {
        self.start.saturating_add(self.size)
    }

    /// Does this sector hold an address
    pub fn contains(&self, address: u32) -> bool {
        address >= self.start && address < self.end()
    }
}

impl std::fmt::Display for Sector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "sector {} at {:#010x}, {} bytes",
            self.index, self.start, self.size
        )
    }
}

/// The STM32F40x and F41x memory layout, which is what the TYT radios use
pub const STM32F40X: &[Sector] = &[
    Sector {
        index: 0,
        start: 0x0800_0000,
        size: 0x4000,
    },
    Sector {
        index: 1,
        start: 0x0800_4000,
        size: 0x4000,
    },
    Sector {
        index: 2,
        start: 0x0800_8000,
        size: 0x4000,
    },
    Sector {
        index: 3,
        start: 0x0800_c000,
        size: 0x4000,
    },
    Sector {
        index: 4,
        start: 0x0801_0000,
        size: 0x1_0000,
    },
    Sector {
        index: 5,
        start: 0x0802_0000,
        size: 0x2_0000,
    },
    Sector {
        index: 6,
        start: 0x0804_0000,
        size: 0x2_0000,
    },
    Sector {
        index: 7,
        start: 0x0806_0000,
        size: 0x2_0000,
    },
    Sector {
        index: 8,
        start: 0x0808_0000,
        size: 0x2_0000,
    },
    Sector {
        index: 9,
        start: 0x080a_0000,
        size: 0x2_0000,
    },
    Sector {
        index: 10,
        start: 0x080c_0000,
        size: 0x2_0000,
    },
    Sector {
        index: 11,
        start: 0x080e_0000,
        size: 0x2_0000,
    },
];

/// Which sector holds an address
pub fn sector_for(map: &[Sector], address: u32) -> Option<&Sector> {
    map.iter().find(|s| s.contains(address))
}

/// One piece of a range, clipped to a single sector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// Where this piece starts
    pub address: u32,
    /// How many bytes of it fall in this sector
    pub length: u32,
    /// The sector it falls in
    pub sector: Sector,
}

/// Split a range of memory into pieces, none of which crosses a sector.
///
/// Stops at the first address that is not in the map rather than carrying on,
/// so a range running off the end of flash returns what was mapped instead of
/// silently writing somewhere else.
pub fn split(map: &[Sector], start: u32, end: u32) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut address = start;

    while address < end {
        let Some(sector) = sector_for(map, address) else {
            break;
        };

        let length = end.min(sector.end()).saturating_sub(address);
        if length == 0 {
            break;
        }

        pieces.push(Piece {
            address,
            length,
            sector: *sector,
        });
        address = address.saturating_add(length);
    }

    pieces
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
    fn the_map_is_contiguous_and_ordered() {
        let mut expected = 0x0800_0000u32;
        for (i, sector) in STM32F40X.iter().enumerate() {
            assert_eq!(sector.index as usize, i, "sectors are numbered in order");
            assert_eq!(
                sector.start, expected,
                "sector {i} does not start where the last one ended"
            );
            expected = sector.end();
        }
        // 1 MB of flash
        assert_eq!(expected, 0x0810_0000);
    }

    #[test]
    fn an_address_finds_its_sector() {
        assert_eq!(sector_for(STM32F40X, 0x0800_0000).map(|s| s.index), Some(0));
        assert_eq!(sector_for(STM32F40X, 0x0800_3fff).map(|s| s.index), Some(0));
        assert_eq!(sector_for(STM32F40X, 0x0800_4000).map(|s| s.index), Some(1));
        assert_eq!(sector_for(STM32F40X, 0x0801_0000).map(|s| s.index), Some(4));
        assert_eq!(
            sector_for(STM32F40X, 0x080f_ffff).map(|s| s.index),
            Some(11)
        );

        // outside flash entirely
        assert!(sector_for(STM32F40X, 0x0000_0000).is_none());
        assert!(sector_for(STM32F40X, 0x0810_0000).is_none());
        assert!(sector_for(STM32F40X, 0xffff_ffff).is_none());
    }

    #[test]
    fn a_range_inside_one_sector_is_left_whole() {
        let pieces = split(STM32F40X, 0x0800_0100, 0x0800_0200);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].address, 0x0800_0100);
        assert_eq!(pieces[0].length, 0x100);
        assert_eq!(pieces[0].sector.index, 0);
    }

    #[test]
    fn a_range_is_split_on_every_sector_boundary() {
        // from the middle of sector 0 into sector 2
        let pieces = split(STM32F40X, 0x0800_2000, 0x0800_9000);

        assert_eq!(pieces.len(), 3);
        assert_eq!(
            (pieces[0].address, pieces[0].length, pieces[0].sector.index),
            (0x0800_2000, 0x2000, 0)
        );
        assert_eq!(
            (pieces[1].address, pieces[1].length, pieces[1].sector.index),
            (0x0800_4000, 0x4000, 1)
        );
        assert_eq!(
            (pieces[2].address, pieces[2].length, pieces[2].sector.index),
            (0x0800_8000, 0x1000, 2)
        );
    }

    /// The pieces have to cover the range exactly: a gap leaves memory
    /// unwritten, and an overlap writes twice
    #[test]
    fn the_pieces_cover_the_range_exactly_and_do_not_overlap() {
        for (start, end) in [
            (0x0800_0000u32, 0x0810_0000u32),
            (0x0800_1234, 0x0808_9abc),
            (0x0801_0000, 0x0801_0001),
            (0x080e_0000, 0x0810_0000),
        ] {
            let pieces = split(STM32F40X, start, end);
            assert!(!pieces.is_empty(), "{start:#x}..{end:#x} produced nothing");

            let mut at = start;
            let mut total = 0u32;
            for piece in &pieces {
                assert_eq!(piece.address, at, "a gap or overlap at {at:#x}");
                assert!(
                    piece.sector.contains(piece.address),
                    "a piece outside its own sector"
                );
                assert!(
                    piece.address + piece.length <= piece.sector.end(),
                    "a piece crossing a sector boundary"
                );
                at += piece.length;
                total += piece.length;
            }
            assert_eq!(at, end, "the pieces stop short of the end");
            assert_eq!(total, end - start, "the pieces do not add up");
        }
    }

    #[test]
    fn a_range_past_the_end_of_flash_stops_at_the_end() {
        let pieces = split(STM32F40X, 0x080f_0000, 0x0820_0000);
        let last = pieces.last().expect("some of it is mapped");
        assert_eq!(
            last.address + last.length,
            0x0810_0000,
            "it must stop where flash does, not carry on"
        );
    }

    #[test]
    fn a_range_entirely_outside_flash_produces_nothing() {
        assert!(split(STM32F40X, 0x2000_0000, 0x2001_0000).is_empty());
        assert!(split(STM32F40X, 0x0810_0000, 0x0820_0000).is_empty());
    }

    #[test]
    fn an_empty_or_backwards_range_produces_nothing() {
        assert!(split(STM32F40X, 0x0800_0000, 0x0800_0000).is_empty());
        assert!(split(STM32F40X, 0x0800_4000, 0x0800_0000).is_empty());
    }
}
