//! Tests against reference files produced by the C++ radio_tool.
//!
//! The fixtures in `tests/fixtures` were written by the C++ implementation
//! that has been flashing real radios for years, captured with
//! `rust/tools/make_fixtures.py`. They are committed, so these tests need no
//! C++ toolchain, no cmake and no `radio_tool` binary. Once the C++ is gone
//! the fixtures remain as the record of what the radios accept.
//!
//! Three things are checked per fixture:
//!
//! 1. we parse it back to exactly the firmware that went in
//! 2. for the deterministic containers, our writer reproduces it byte for byte
//! 3. for SGL, whose header carries values the writer picks at random,
//!    re-serialising a parsed file reproduces it byte for byte, which pins our
//!    header writer against a real C++ file just as tightly
//!
//! If a fixture ever needs to move, that is a deliberate format change:
//! regenerate with the script and commit the result.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use firmware::ailunce::AilunceFirmware;
use firmware::cs::CsFirmware;
use firmware::sgl::SglFirmware;
use firmware::tyt::TytFirmware;
use firmware::yaesu::YaesuFirmware;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const MANIFEST: &str = include_str!("fixtures/manifest.tsv");

/// One reference file and the firmware that went into it
struct Case {
    kind: String,
    model: String,
    address: u32,
    length: usize,
    seed: u32,
    file: String,
    /// Which implementation wrote it. Files from another project pin the
    /// format itself rather than one tool's reading of it.
    source: String,
}

impl Case {
    /// The firmware data this fixture was built from
    fn input(&self) -> Vec<u8> {
        sample(self.length, self.seed)
    }

    /// The reference file as the C++ tool wrote it
    fn bytes(&self) -> Vec<u8> {
        let path = format!("{FIXTURES}/{}", self.file);
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {path}: {e}"))
    }
}

/// Must match `sample` in the unit tests and in make_fixtures.py
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

fn parse_u32(text: &str) -> u32 {
    let text = text.trim();
    match text.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse(),
    }
    .unwrap_or_else(|e| panic!("bad number in the manifest: {text}: {e}"))
}

fn cases() -> Vec<Case> {
    let cases: Vec<Case> = MANIFEST
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(f.len(), 7, "malformed manifest line: {line}");
            Case {
                kind: f[0].to_owned(),
                model: f[1].to_owned(),
                address: parse_u32(f[2]),
                length: parse_u32(f[3]) as usize,
                seed: parse_u32(f[4]),
                file: f[5].to_owned(),
                source: f[6].to_owned(),
            }
        })
        .collect();

    assert!(!cases.is_empty(), "the fixture manifest is empty");
    cases
}

/// Where two files first differ, for a readable failure
fn describe_difference(want: &[u8], got: &[u8]) -> String {
    let at = if want.len() != got.len() {
        want.len().min(got.len())
    } else {
        match want.iter().zip(got).position(|(a, b)| a != b) {
            None => return "identical".to_owned(),
            Some(at) => at,
        }
    };
    let window = |buf: &[u8]| {
        let end = (at + 8).min(buf.len());
        buf.get(at..end)
            .unwrap_or_default()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!(
        "first difference at {at:#x} (reference {} bytes, ours {} bytes)\n  reference: {}\n  ours:      {}",
        want.len(),
        got.len(),
        window(want),
        window(got)
    )
}

#[test]
fn the_manifest_covers_every_fixture_on_disk() {
    let listed: std::collections::HashSet<String> = cases().into_iter().map(|c| c.file).collect();

    let mut found = 0;
    for entry in std::fs::read_dir(FIXTURES).expect("the fixture directory exists") {
        let name = entry.expect("readable entry").file_name();
        let name = name.to_string_lossy().into_owned();
        if name == "manifest.tsv" || name == "README.md" {
            continue;
        }
        assert!(listed.contains(&name), "{name} is not in the manifest");
        found += 1;
    }

    assert_eq!(
        found,
        listed.len(),
        "the manifest lists files that do not exist"
    );
}

#[test]
fn we_parse_every_reference_file_back_to_the_firmware_that_went_in() {
    for case in cases() {
        let file = case.bytes();
        let want = case.input();
        let name = &case.file;

        match case.kind.as_str() {
            "tyt" => {
                assert!(TytFirmware::is_supported(&file), "{name}: not recognised");
                let fw = TytFirmware::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));

                // Some radios share a counter magic, so a file cannot always
                // name the radio it was built for: an MD446 file reads back
                // as an MD380, in the C++ tool too. The invariant is that the
                // radio we resolve is one the file cannot be told apart from.
                let wanted = firmware::tyt::config_for_model(&case.model)
                    .unwrap_or_else(|| panic!("{name}: {} is not a known model", case.model));
                assert_eq!(
                    fw.config().counter_magic,
                    wanted.counter_magic,
                    "{name}: resolved to {}, which is a different radio",
                    fw.config().radio_model
                );

                let segments = fw.segments();
                assert_eq!(segments.len(), 1, "{name}: wrong segment count");
                assert_eq!(segments[0].address, case.address, "{name}: wrong address");
                assert_eq!(segments[0].data, want, "{name}: firmware data differs");
            }
            "sgl" => {
                assert!(SglFirmware::is_supported(&file), "{name}: not recognised");
                let fw = SglFirmware::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(fw.config().radio_model, case.model, "{name}: wrong radio");
                assert_eq!(fw.data(), want, "{name}: firmware data differs");
            }
            "cs" => {
                assert!(CsFirmware::is_supported(&file), "{name}: not recognised");
                // parse validates the checksum
                let fw = CsFirmware::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(fw.base_address(), case.address, "{name}: wrong address");
                assert_eq!(fw.data(), want, "{name}: firmware data differs");
            }
            "ailunce" => {
                let fw = AilunceFirmware::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));

                // the obfuscation cannot round trip a trailing partial word,
                // so only compare the part that can survive
                let whole = want.len() - (want.len() % 4);
                assert_eq!(
                    &fw.data()[..whole],
                    &want[..whole],
                    "{name}: firmware data differs"
                );
            }
            "yaesu" => {
                let fw = YaesuFirmware::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(fw.data(), want, "{name}: firmware data differs");
            }
            other => panic!("{name}: unknown container kind {other}"),
        }
    }
}

#[test]
fn our_writer_reproduces_the_deterministic_reference_files() {
    let mut checked = 0;

    for case in cases() {
        let want = case.bytes();
        let data = case.input();
        let name = &case.file;

        // another implementation is free to differ in fields the radio does
        // not care about, so only our own reference files are compared byte
        // for byte. Reading them is checked for every source.
        if case.source != "radio_tool" {
            continue;
        }

        let got = match case.kind.as_str() {
            "tyt" => {
                let mut fw = TytFirmware::new(&case.model).expect("the model is supported");
                fw.append_segment(case.address, &data, 0)
                    .expect("the segment fits");
                fw.serialise().expect("serialises")
            }
            "cs" => {
                let mut fw = CsFirmware::new(&case.model).expect("the model is supported");
                fw.set_segment(case.address, &data).expect("the data fits");
                fw.serialise().expect("serialises")
            }
            "ailunce" => {
                let mut fw = AilunceFirmware::new(&case.model).expect("the model is supported");
                fw.set_data(&data).expect("the data fits");
                fw.serialise().expect("serialises")
            }
            "yaesu" => {
                let mut fw = YaesuFirmware::new(&case.model).expect("the model is supported");
                fw.set_data(&data).expect("the data fits");
                fw.serialise().expect("serialises")
            }
            // SGL headers carry values the writer picks at random, so there is
            // nothing deterministic to compare. Covered by the test below.
            _ => continue,
        };

        // We deliberately differ from radio_tool in the model name padding:
        // it zero fills, genuine vendor firmware writes two nulls then 0xff,
        // and we follow the vendor. See the test below. Everything else has
        // to match byte for byte.
        let ignore = if case.kind == "tyt" { 0x10..0x20 } else { 0..0 };
        let mask = |buf: &[u8]| {
            let mut out = buf.to_vec();
            for byte in out.get_mut(ignore.clone()).unwrap_or_default() {
                *byte = 0;
            }
            out
        };

        assert!(
            mask(&want) == mask(&got),
            "{name}: our output differs from the C++ reference\n{}",
            describe_difference(&want, &got)
        );
        checked += 1;
    }

    assert!(checked > 0, "no deterministic fixtures were checked");
}

#[test]
fn rewriting_a_parsed_sgl_reference_file_reproduces_it_exactly() {
    let mut checked = 0;

    for case in cases().into_iter().filter(|c| c.kind == "sgl") {
        let want = case.bytes();
        let name = &case.file;

        let fw = SglFirmware::parse(&want).unwrap_or_else(|e| panic!("{name}: {e}"));
        let got = fw.serialise().expect("serialises");

        assert!(
            want == got,
            "{name}: rewriting a C++ written file changed it\n{}",
            describe_difference(&want, &got)
        );
        checked += 1;
    }

    assert!(checked > 0, "no SGL fixtures were checked");
}

#[test]
fn a_corrupt_reference_file_is_always_rejected() {
    // the fixtures are the only real firmware files we have, so use them to
    // check that corruption is caught rather than quietly accepted
    for case in cases() {
        let good = case.bytes();
        let name = &case.file;

        for at in [0usize, 4, 0x40, 0x90, 0x110] {
            if at >= good.len() {
                continue;
            }
            let mut bad = good.clone();
            bad[at] ^= 0xff;

            // must not panic, whatever it decides
            match case.kind.as_str() {
                "tyt" => {
                    let _ = TytFirmware::parse(&bad);
                }
                "sgl" => {
                    let _ = SglFirmware::parse(&bad);
                }
                "cs" => {
                    // a file with a checksum must catch corruption in the
                    // firmware data. A CSFWTOOL file carries no checksum, so
                    // there is nothing it could be caught with.
                    let result = CsFirmware::parse(&bad);
                    if at >= 0x80 && case.source == "radio_tool" {
                        assert!(
                            result.is_err(),
                            "{name}: corruption at {at:#x} was accepted"
                        );
                    }
                }
                // headerless and unchecksummed, so there is nothing to detect
                // corruption with. All that matters is that it does not panic.
                "ailunce" => {
                    let _ = AilunceFirmware::parse(&bad);
                }
                "yaesu" => {
                    let _ = YaesuFirmware::parse(&bad);
                }
                other => panic!("{name}: unknown container kind {other}"),
            }
        }
    }
}

/// Files written by a project that reverse engineered the format separately.
///
/// Agreeing with radio_tool only shows we ported radio_tool faithfully. It
/// says nothing about whether radio_tool understood the format. A file from
/// another team, working from the same radios but not the same source, is a
/// second opinion on the format itself.
#[test]
fn we_read_files_written_by_other_implementations() {
    let mut checked = 0;

    for case in cases().into_iter().filter(|c| c.source != "radio_tool") {
        let file = case.bytes();
        let want = case.input();
        let name = &case.file;

        match case.kind.as_str() {
            "tyt" => {
                assert!(
                    TytFirmware::is_supported(&file),
                    "{name}: we did not recognise a file from {}",
                    case.source
                );
                let fw = TytFirmware::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
                let segments = fw.segments();

                assert_eq!(segments.len(), 1, "{name}: wrong segment count");
                assert_eq!(segments[0].address, case.address, "{name}: wrong address");
                assert_eq!(
                    segments[0].data, want,
                    "{name}: firmware from {} decoded differently",
                    case.source
                );
            }
            "cs" => {
                // CSFWTOOL writes no trailing checksum, which we accept
                assert!(
                    CsFirmware::is_supported(&file),
                    "{name}: we did not recognise a file from {}",
                    case.source
                );
                let fw = CsFirmware::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(
                    !fw.had_checksum(),
                    "{name}: CSFWTOOL is not supposed to write a checksum"
                );
                assert_eq!(fw.base_address(), case.address, "{name}: wrong address");
                assert_eq!(
                    fw.data(),
                    want,
                    "{name}: firmware from {} decoded differently",
                    case.source
                );
            }
            other => panic!("{name}: unknown third party container kind {other}"),
        }
        checked += 1;
    }

    assert!(checked > 0, "no third party fixtures were checked");
}

/// md380tools leaves the region count as 0xffffffff, because the field falls
/// inside a run of padding it never fills in. Reading such a file as having
/// one region is the only reason radio_tool special cases that value, so it
/// is worth pinning to a real example rather than a synthetic one.
#[test]
fn a_third_party_file_exercises_the_ffffffff_region_count() {
    let case = cases()
        .into_iter()
        .find(|c| c.source == "md380tools")
        .expect("the md380tools fixture is present");

    let file = case.bytes();
    assert_eq!(
        &file[0x7c..0x80],
        &[0xff, 0xff, 0xff, 0xff],
        "this fixture is supposed to carry an unset region count"
    );

    let fw = TytFirmware::parse(&file).expect("parses");
    assert_eq!(fw.segments().len(), 1, "0xffffffff must read as one region");
}

/// The model name field, where we knowingly differ from radio_tool.
///
/// Genuine TYT firmware writes the model, two nulls, then 0xff to the end of
/// the 16 byte field. md380tools does the same. radio_tool zero fills, which
/// no real firmware file does, so this port follows the vendor. Confirmed
/// against 14 real firmware files, every one of which then round trips byte
/// for byte.
#[test]
fn we_write_the_model_name_field_the_way_the_vendor_does() {
    let reference = cases()
        .into_iter()
        .find(|c| c.file == "tyt_MD380.bin")
        .expect("the MD380 fixture");

    // the same firmware the fixture was built from, so the payloads match and
    // only the header can differ
    let mut fw = TytFirmware::new("MD380").expect("the model is supported");
    fw.append_segment(reference.address, &reference.input(), 0)
        .expect("the segment fits");
    let ours = fw.serialise().expect("serialises");

    assert_eq!(&ours[0x10..0x15], b"JST51", "the model name comes first");
    assert_eq!(&ours[0x15..0x17], &[0x00, 0x00], "then exactly two nulls");
    assert!(
        ours[0x17..0x20].iter().all(|b| *b == 0xff),
        "then 0xff to the end of the field, not zeroes"
    );

    // and the radio_tool reference really does differ, only there
    let theirs = reference.bytes();
    let differing: Vec<usize> = theirs
        .iter()
        .zip(&ours)
        .enumerate()
        .filter_map(|(ix, (a, b))| (a != b).then_some(ix))
        .collect();
    assert!(
        differing.iter().all(|ix| (0x10..0x20).contains(ix)),
        "the difference from radio_tool must be confined to the model field, got {differing:x?}"
    );
    assert!(
        !differing.is_empty(),
        "there is supposed to be a difference"
    );
}
