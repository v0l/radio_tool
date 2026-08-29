//! Tests against a real UV-5R image.
//!
//! `uv5r_hn5rv01.img` was downloaded from the simulated radio in
//! `test/fake_uv5r.py` using the C++ radio_tool, so it is a genuine 6472 byte
//! clone image rather than something constructed here. It is committed, so
//! this test needs no radio and no C++ toolchain.
//!
//! The channel contents are the ones the C++ shell test checks for, which
//! makes this the same assertion in a different language: if the port ever
//! disagrees with the C++ about what is in this image, one of them is wrong.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use codeplug::uv5r::{Power, Tone, Uv5rCodeplug};

const IMAGE: &[u8] = include_bytes!("fixtures/uv5r_hn5rv01.img");

fn parsed() -> Uv5rCodeplug {
    Uv5rCodeplug::parse(IMAGE).expect("the reference image parses")
}

#[test]
fn the_image_is_the_shape_we_expect() {
    // eight byte ident, 0x1800 of main memory, 0x140 of aux
    assert_eq!(IMAGE.len(), 0x1948);
    assert_eq!(&IMAGE[..3], &[0x50, 0xbb, 0xff]);
    assert!(Uv5rCodeplug::is_supported(IMAGE));
}

#[test]
fn the_radio_strings_are_read() {
    let cp = parsed();
    assert_eq!(cp.firmware_version(), "HN5RV01");
    assert_eq!(cp.power_on_message(), "HELLO / RADIO");
}

#[test]
fn three_channels_are_programmed() {
    let cp = parsed();
    assert_eq!(cp.channels().len(), 128);

    let used: Vec<usize> = cp.used_channels().map(|(ix, _)| ix).collect();
    assert_eq!(used, vec![0, 1, 2], "channels 0 to 2 are programmed");
}

#[test]
fn a_simplex_channel_reads_back_correctly() {
    let cp = parsed();
    let (_, ch) = cp.used_channels().next().expect("channel 0");

    assert_eq!(ch.name, "SIMPLEX");
    assert_eq!(ch.rx_freq, 145_500_000);
    assert_eq!(
        ch.tx_freq, 145_500_000,
        "simplex transmits where it listens"
    );
    assert!(!ch.tx_inhibit);
    assert_eq!(ch.rx_tone, Tone::None);
    assert_eq!(ch.tx_tone, Tone::None);
    assert!(ch.wide);
    assert!(ch.scan);
    assert_eq!(ch.power, Power::High);
}

#[test]
fn a_repeater_channel_has_a_split_and_a_ctcss_tone() {
    let cp = parsed();
    let (_, ch) = cp.used_channels().nth(1).expect("channel 1");

    assert_eq!(ch.name, "RPT1");
    assert_eq!(ch.rx_freq, 145_625_000);
    assert_eq!(ch.tx_freq, 145_025_000, "a 600 kHz negative split");
    assert_eq!(ch.rx_tone, Tone::None);
    assert_eq!(ch.tx_tone, Tone::Ctcss { tenths: 717 });
    assert_eq!(ch.tx_tone.to_string(), "71.7");
}

#[test]
fn a_narrow_low_power_channel_has_dtcs_on_both_ends() {
    let cp = parsed();
    let (_, ch) = cp.used_channels().nth(2).expect("channel 2");

    assert_eq!(ch.name, "PMR1");
    assert_eq!(ch.rx_freq, 446_006_250, "a PMR446 channel");
    assert_eq!(ch.tx_freq, 446_006_250);
    assert_eq!(
        ch.rx_tone,
        Tone::Dtcs {
            code: 132,
            inverted: false
        }
    );
    assert_eq!(ch.tx_tone, ch.rx_tone);
    assert_eq!(ch.rx_tone.to_string(), "D132N");
    assert!(!ch.wide, "PMR446 is narrow band");
    assert_eq!(ch.power, Power::Low, "PMR446 is low power");
}

/// The C++ prints this image with `--codeplug-info`, and its shell test greps
/// the output for these lines. Ours has to say the same things.
#[test]
fn the_summary_says_what_the_cpp_says() {
    let text = parsed().to_string();

    for want in [
        "Baofeng UV-5R Codeplug",
        "HN5RV01",
        "HELLO / RADIO",
        "SIMPLEX     145.50000  145.50000",
        "RPT1        145.62500  145.02500",
        "PMR1        446.00625  446.00625",
        "3 of 128 channels used",
    ] {
        assert!(text.contains(want), "missing {want:?} from:\n{text}");
    }
}

#[test]
fn every_truncation_and_corruption_is_handled() {
    for len in 0..IMAGE.len() {
        let _ = Uv5rCodeplug::parse(&IMAGE[..len]);
        let _ = Uv5rCodeplug::is_supported(&IMAGE[..len]);
    }

    // corrupt each byte of the first few channels and the strings
    for at in (0..0x60).chain(0x1820..0x1850) {
        let mut bad = IMAGE.to_vec();
        bad[at] ^= 0xff;
        if let Ok(cp) = Uv5rCodeplug::parse(&bad) {
            let _ = cp.to_string();
        }
    }
}

// ---------------------------------------------------------------------------
// RDT
//
// dm1701_synthetic.rdt is built the way the C++ test_codeplug.cpp builds its
// fixture, and the C++ radio_tool prints the field values asserted below for
// it. Layout and offsets were separately confirmed against dmrconfig.
// ---------------------------------------------------------------------------

use codeplug::rdt::RdtCodeplug;

const RDT: &[u8] = include_bytes!("fixtures/dm1701_synthetic.rdt");

#[test]
fn the_rdt_header_is_recognised() {
    assert!(RdtCodeplug::is_supported(RDT));
    assert_eq!(&RDT[..5], b"DfuSe");
    assert_eq!(&RDT[0x0b..0x11], b"Target");
}

#[test]
fn the_rdt_fields_match_what_the_cpp_prints() {
    let cp = RdtCodeplug::parse(RDT).expect("parses");

    // the C++ prints: Radio: DM-1701, Target: MD-1701 codeplug,
    // Created: Tue Oct 26 08:56:55 2021, Name: MYRADIO, Radio ID: 0,
    // Intro 1: HELLO, Intro 2: WORLD
    assert_eq!(cp.radio(), "DM-1701");
    assert_eq!(cp.target_name(), "MD-1701 codeplug");

    let ts = cp.timestamp().expect("a valid date");
    assert_eq!(ts.to_string(), "2021-10-26 08:56:55");

    let g = cp.general();
    assert_eq!(g.radio_name, "MYRADIO");
    assert_eq!(g.radio_id, 0);
    assert_eq!(g.intro_line1, "HELLO");
    assert_eq!(g.intro_line2, "WORLD");
}

#[test]
fn a_truncated_rdt_never_panics() {
    for len in (0..RDT.len()).step_by(7) {
        let _ = RdtCodeplug::parse(&RDT[..len]);
        let _ = RdtCodeplug::is_supported(&RDT[..len]);
    }
}

// ---------------------------------------------------------------------------
// Real images, from real radios
//
// These come from CHIRP's own test suite, so they were produced by hardware
// rather than by a simulator or by radio_tool. They are the reason this crate
// identifies images the way CHIRP does: neither one carries the 0x50 0xbb
// 0xff ident block that the C++ requires, and the C++ rejects both.
// ---------------------------------------------------------------------------

use codeplug::uv5r::strip_chirp_metadata;

/// Saved by CHIRP, so it carries a metadata trailer
const REAL_UV5R: &[u8] = include_bytes!("fixtures/chirp_baofeng_uv5r.img");
/// A raw image with no trailer, and a binary ident
const REAL_F11: &[u8] = include_bytes!("fixtures/chirp_baofeng_f11.img");

#[test]
fn chirp_metadata_is_stripped() {
    // 0x1948 of image, then CHIRP's magic and a base64 blob
    assert_eq!(REAL_UV5R.len(), 6661);
    assert_eq!(strip_chirp_metadata(REAL_UV5R).len(), 0x1948);

    // a file without a trailer is returned untouched
    assert_eq!(REAL_F11.len(), 0x1948);
    assert_eq!(strip_chirp_metadata(REAL_F11).len(), 0x1948);
}

#[test]
fn real_images_do_not_carry_the_ident_the_cpp_expects() {
    // this is why identification follows the firmware string instead
    for image in [REAL_UV5R, REAL_F11] {
        assert_ne!(
            &image[..3],
            &[0x50, 0xbb, 0xff],
            "a real radio does not return this ident"
        );
    }
    // the F-11's ident is not even printable
    assert_eq!(&REAL_F11[..2], &[0xaa, 0x36]);
}

#[test]
fn real_images_are_recognised_and_parse() {
    for (image, fw) in [(REAL_UV5R, "BFB231"), (REAL_F11, "USA307")] {
        assert!(Uv5rCodeplug::is_supported(image), "not recognised");

        let cp = Uv5rCodeplug::parse(image).expect("a real image parses");
        assert!(
            cp.firmware_version().contains(fw),
            "expected {fw} in {:?}",
            cp.firmware_version()
        );
        assert_eq!(cp.channels().len(), 128);
    }
}

#[test]
fn a_real_uv5r_holds_the_channels_it_should() {
    let cp = Uv5rCodeplug::parse(REAL_UV5R).expect("parses");
    assert_eq!(cp.power_on_message(), "AllGood / Now");

    // a US 70cm repeater pair with DCS on both ends
    let (ix, ch) = cp
        .used_channels()
        .find(|(_, c)| c.name == "HTAC1")
        .expect("HTAC1 is programmed");
    assert_eq!(ix, 25);
    assert_eq!(ch.rx_freq, 443_000_000);
    assert_eq!(ch.tx_freq, 448_000_000, "a 5 MHz repeater split");
    assert_eq!(
        ch.rx_tone,
        Tone::Dtcs {
            code: 23,
            inverted: false
        }
    );
    assert_eq!(ch.power, Power::Low);

    // a 2m channel with a CTCSS tone only on transmit
    let (_, ch) = cp
        .used_channels()
        .find(|(_, c)| c.name == "HTAC3")
        .expect("HTAC3 is programmed");
    assert_eq!(ch.rx_freq, 147_440_000);
    assert_eq!(ch.rx_tone, Tone::None);
    assert_eq!(ch.tx_tone, Tone::Ctcss { tenths: 885 });
}

#[test]
fn real_images_survive_truncation_and_corruption() {
    for image in [REAL_UV5R, REAL_F11] {
        for len in (0..image.len()).step_by(11) {
            let _ = Uv5rCodeplug::parse(&image[..len]);
            let _ = Uv5rCodeplug::is_supported(&image[..len]);
        }
        for at in (0..0x200).chain(0x1830..0x1850) {
            let mut bad = image.to_vec();
            bad[at] ^= 0xff;
            if let Ok(cp) = Uv5rCodeplug::parse(&bad) {
                let _ = cp.to_string();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UV-17Pro family
//
// chirp_baofeng_uv5r_mini.img is CHIRP's own test image for the UV-5R Mini,
// so it came off real hardware. A codeplug read from a real UV-5R Mini over
// Bluetooth by this crate parses the same way, but that one is somebody's
// actual channel memory and is not committed.
// ---------------------------------------------------------------------------

use codeplug::uv17pro::Uv17ProCodeplug;

const REAL_MINI: &[u8] = include_bytes!("fixtures/chirp_baofeng_uv5r_mini.img");

#[test]
fn a_real_uv5r_mini_image_is_recognised() {
    assert!(Uv17ProCodeplug::is_supported(REAL_MINI));

    let model = Uv17ProCodeplug::identify(REAL_MINI).expect("identified");
    assert_eq!(model.memory_size, 0x8240, "the UV-5R Mini memory size");
    assert_eq!(model.channels, 999);
}

#[test]
fn a_real_uv5r_mini_image_parses() {
    let cp = Uv17ProCodeplug::parse(REAL_MINI).expect("parses");
    assert_eq!(cp.channels().len(), 999);

    let used: Vec<_> = cp.used_channels().collect();
    assert!(!used.is_empty(), "the image has programmed channels");

    // every programmed channel must have a frequency that could be real
    for (ix, ch) in &used {
        assert!(
            (100_000_000..1_000_000_000).contains(&ch.rx_freq),
            "channel {ix} has an implausible frequency {}",
            ch.rx_freq
        );
    }

    // the first is a 2m channel, which is what CHIRP's image holds
    let (_, first) = used.first().expect("at least one channel");
    assert_eq!(first.rx_freq, 144_925_000);
}

/// The bandwidth bit is named `wide` in CHIRP's struct and used inverted.
/// CHIRP's own image has it clear on a 2m channel, which is wide FM, and a
/// real radio programmed with PMR446 channels has it set, which is narrow.
#[test]
fn bandwidth_matches_what_chirp_would_report() {
    use codeplug::uv17pro::Mode;

    let cp = Uv17ProCodeplug::parse(REAL_MINI).expect("parses");
    let (_, first) = cp.used_channels().next().expect("a channel");

    assert_eq!(first.rx_freq, 144_925_000);
    assert_eq!(
        first.mode,
        Mode::Wide,
        "the bit is clear on this channel, and clear means wide"
    );
}

#[test]
fn the_uv17pro_family_is_not_confused_with_the_classic_uv5r() {
    // the two formats are different radios with overlapping names, and an
    // image of one must never parse as the other
    assert!(!Uv5rCodeplug::is_supported(REAL_MINI));
    assert!(!Uv17ProCodeplug::is_supported(REAL_UV5R));
    assert!(!Uv17ProCodeplug::is_supported(REAL_F11));
}

#[test]
fn a_mini_image_survives_truncation_and_corruption() {
    for len in (0..REAL_MINI.len()).step_by(97) {
        let _ = Uv17ProCodeplug::parse(&REAL_MINI[..len]);
        let _ = Uv17ProCodeplug::is_supported(&REAL_MINI[..len]);
    }
    for at in (0..0x400).chain(0x8230..0x8240) {
        let mut bad = REAL_MINI.to_vec();
        bad[at] ^= 0xff;
        if let Ok(cp) = Uv17ProCodeplug::parse(&bad) {
            let _ = cp.to_string();
        }
    }
}

// ---------------------------------------------------------------------------
// The format registry
//
// These run over codeplug::FORMATS rather than over a hand written list, so a
// format added later is covered without anyone remembering to add it here.
// ---------------------------------------------------------------------------

/// Every real image, paired with the format that should claim it
fn real_images() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("UV-5R", REAL_UV5R),
        ("UV-5R", REAL_F11),
        ("UV-17Pro", REAL_MINI),
        ("RDT", RDT),
    ]
}

#[test]
fn each_real_image_is_claimed_by_exactly_one_format() {
    for (expected, image) in real_images() {
        let claimants: Vec<&str> = codeplug::FORMATS
            .iter()
            .filter(|f| (f.is_supported)(image))
            .map(|f| f.name)
            .collect();

        assert_eq!(
            claimants,
            vec![expected],
            "an image should be claimed by one format and only one"
        );
    }
}

#[test]
fn the_registry_reads_every_real_image() {
    for (expected, image) in real_images() {
        let parsed = codeplug::parse(image).expect("the registry reads it");
        assert_eq!(parsed.format(), expected);
        assert!(!parsed.radio().is_empty(), "it says what radio it is for");
        assert!(!parsed.to_string().is_empty(), "it describes itself");
    }
}

#[test]
fn nothing_is_claimed_as_a_codeplug_by_accident() {
    // firmware files, random bytes, and truncations must all be refused
    for junk in [
        &b""[..],
        &[0u8; 64][..],
        &[0xff; 4096][..],
        b"#!/bin/sh\nexit 1\n",
    ] {
        assert!(
            codeplug::identify(junk).is_none(),
            "something claimed {} bytes of junk",
            junk.len()
        );
    }
}

#[test]
fn a_truncated_image_is_never_silently_claimed_by_another_format() {
    for (expected, image) in real_images() {
        for len in (0..image.len()).step_by(211) {
            if let Some(format) = codeplug::identify(&image[..len]) {
                assert_eq!(
                    format.name, expected,
                    "a truncated {expected} image was claimed by {}",
                    format.name
                );
            }
        }
    }
}
