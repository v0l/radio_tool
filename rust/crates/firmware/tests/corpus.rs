//! Round trip every real vendor firmware file we can get hold of.
//!
//! The corpus is not committed: it is vendor property and runs to megabytes.
//! Recover it first, which verifies every file against the sha256 manifest
//! that survived in `test/firmware`:
//!
//! ```sh
//! python3 test/firmware/recover.py --download
//! cargo test -p firmware --test corpus
//! ```
//!
//! This test skips when the corpus is absent, which is safe because the
//! committed fixtures already cover every format. What the corpus adds is
//! depth: real files, written by the vendor's own tools, in sizes and shapes
//! nobody here chose. Set `RADIO_TOOL_CORPUS_REQUIRED=1` to make an absent
//! corpus a failure instead.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use firmware::cs::CsFirmware;
use firmware::sgl::SglFirmware;
use firmware::tyt::TytFirmware;
use std::path::{Path, PathBuf};

fn corpus_dir() -> Option<PathBuf> {
    let dir = match std::env::var_os("RADIO_TOOL_CORPUS") {
        Some(path) => PathBuf::from(path),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("the crate lives three levels below the repo root")
            .join("test/firmware/data"),
    };

    if dir.is_dir() {
        return Some(dir);
    }
    if std::env::var_os("RADIO_TOOL_CORPUS_REQUIRED").is_some() {
        panic!(
            "RADIO_TOOL_CORPUS_REQUIRED is set but {} does not exist. \
             Run: python3 test/firmware/recover.py --download",
            dir.display()
        );
    }
    None
}

/// Every real firmware file must survive parse, decrypt, re-encrypt and
/// re-serialise with not one byte changed.
///
/// This is the strongest check in the crate. Agreeing with another tool shows
/// we read the format the way that tool does; reproducing a vendor file shows
/// we read it the way the radio does.
#[test]
fn real_vendor_firmware_round_trips_byte_for_byte() {
    let Some(dir) = corpus_dir() else {
        eprintln!("skipping: no firmware corpus, see test/firmware/recover.py");
        return;
    };

    let mut checked = 0;
    let mut skipped = 0;

    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the corpus directory is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();

    for path in paths {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let data = std::fs::read(&path).expect("a corpus file is readable");

        // whichever container it is, reading and writing it must not change
        // a single byte
        let again = if TytFirmware::is_supported(&data) {
            let fw = TytFirmware::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!fw.segments().is_empty(), "{name}: no segments");
            assert_eq!(
                fw.segments().iter().map(|s| s.data.len()).sum::<usize>(),
                fw.data().len(),
                "{name}: segments do not cover the firmware"
            );
            fw.serialise().unwrap_or_else(|e| panic!("{name}: {e}"))
        } else if SglFirmware::is_supported(&data) {
            let fw = SglFirmware::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!fw.data().is_empty(), "{name}: no firmware data");
            fw.serialise().unwrap_or_else(|e| panic!("{name}: {e}"))
        } else if CsFirmware::is_supported(&data) {
            let fw = CsFirmware::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!fw.data().is_empty(), "{name}: no firmware data");
            fw.serialise().unwrap_or_else(|e| panic!("{name}: {e}"))
        } else {
            skipped += 1;
            continue;
        };

        assert_eq!(again.len(), data.len(), "{name}: length changed");
        let at = again.iter().zip(&data).position(|(a, b)| a != b);
        assert!(
            at.is_none(),
            "{name}: rewriting a vendor file changed byte {:#x}",
            at.unwrap_or(0)
        );

        checked += 1;
    }

    eprintln!("{checked} vendor firmware files round tripped, {skipped} unrecognised");
    assert!(checked > 0, "the corpus held no recognised firmware");
}

/// Recover the XOR key from real firmware, knowing nothing about the radio.
///
/// This is how the keys in `cipher` were found in the first place, and real
/// firmware is the only fair test of it: synthetic data either makes it too
/// easy or, if the content lands on a stride that divides the key length,
/// impossible. Recovering a known key from a vendor file shows the technique
/// still works and that our stored key is the one really in use.
#[test]
fn the_key_guesser_recovers_known_keys_from_real_firmware() {
    let Some(dir) = corpus_dir() else {
        eprintln!("skipping: no firmware corpus, see test/firmware/recover.py");
        return;
    };

    let mut checked = 0;

    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the corpus directory is readable")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();

    for path in paths {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let data = std::fs::read(&path).expect("readable");
        if !TytFirmware::is_supported(&data) {
            continue;
        }

        let fw = TytFirmware::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        let encrypted =
            firmware::tyt::encrypted_payload(&data).unwrap_or_else(|e| panic!("{name}: {e}"));

        // a firmware has to be big enough for each key position to see a fair
        // number of bytes before the guess means anything
        if encrypted.len() < 64 * firmware::keyguess::KEY_LEN {
            continue;
        }

        let guess = firmware::keyguess::guess_key(encrypted);
        let agreement = firmware::keyguess::key_agreement(&guess, fw.config().cipher);

        // Measured range over this corpus is 60% to 100%, median 65%: dense
        // code recovers worse than an image with a resource region. Chance is
        // 1 in 256, so half is still two orders of magnitude better than
        // guessing, and a collapse to noise would mean the guesser is broken.
        assert!(
            agreement > 0.5,
            "{name}: guessed key agrees with the known {} key only {:.1}% of the time",
            fw.config().radio_model,
            agreement * 100.0
        );
        checked += 1;
    }

    eprintln!("{checked} firmware files gave up their key to frequency analysis");
    assert!(checked > 0, "no firmware was large enough to guess from");
}
