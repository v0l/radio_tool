//! End to end tests of the command line tool.
//!
//! These run the built binary the way a user would, against the committed
//! fixtures in the library crates, so they cover argument handling and output
//! as well as the parsing underneath.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

fn tool() -> &'static str {
    env!("CARGO_BIN_EXE_radio-tool")
}

fn fixture(crate_name: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates directory")
        .join(crate_name)
        .join("tests/fixtures")
        .join(name)
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "radio_tool_cli_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        Self(path)
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(tool())
        .args(args)
        .output()
        .expect("the tool runs")
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn fw_info_describes_a_tyt_firmware() {
    let file = fixture("firmware", "tyt_MD9600.bin");
    let out = run(&["fw-info", &file.to_string_lossy()]);
    assert!(out.status.success(), "{}", combined(&out));

    let text = stdout_of(&out);
    assert!(text.contains("TYT Firmware"), "{text}");
    assert!(text.contains("MD9600"), "{text}");
    assert!(text.contains("0x0800c000"), "{text}");
}

#[test]
fn fw_info_describes_an_sgl_firmware() {
    let file = fixture("firmware", "sgl_GD77.bin");
    let out = run(&["fw-info", &file.to_string_lossy()]);
    assert!(out.status.success(), "{}", combined(&out));

    let text = stdout_of(&out);
    assert!(text.contains("SGL Firmware"), "{text}");
    assert!(text.contains("GD77"), "{text}");
    assert!(text.contains("SG-MD-760"), "{text}");
}

#[test]
fn a_headerless_firmware_needs_the_radio_naming() {
    let file = fixture("firmware", "ailunce_HD1_0x400.bin");

    // without a radio there is nothing in the file to go on
    let out = run(&["fw-info", &file.to_string_lossy()]);
    assert!(!out.status.success());
    assert!(combined(&out).contains("--radio"), "{}", combined(&out));

    let out = run(&["fw-info", "--radio", "HD1", &file.to_string_lossy()]);
    assert!(out.status.success(), "{}", combined(&out));
    assert!(stdout_of(&out).contains("Ailunce"));
}

#[test]
fn naming_the_wrong_radio_is_an_error() {
    let file = fixture("firmware", "tyt_MD9600.bin");
    let out = run(&["fw-info", "--radio", "GD77", &file.to_string_lossy()]);

    assert!(!out.status.success());
    assert!(combined(&out).contains("MD9600"), "{}", combined(&out));
}

#[test]
fn codeplug_info_reads_a_real_uv5r_image() {
    let file = fixture("codeplug", "chirp_baofeng_uv5r.img");
    let out = run(&["codeplug-info", &file.to_string_lossy()]);
    assert!(out.status.success(), "{}", combined(&out));

    let text = stdout_of(&out);
    assert!(text.contains("UV-5R Codeplug"), "{text}");
    assert!(text.contains("BFB231"), "{text}");
    assert!(text.contains("HTAC1"), "{text}");
}

#[test]
fn codeplug_info_reads_an_rdt() {
    let file = fixture("codeplug", "dm1701_synthetic.rdt");
    let out = run(&["codeplug-info", &file.to_string_lossy()]);
    assert!(out.status.success(), "{}", combined(&out));

    let text = stdout_of(&out);
    assert!(text.contains("RDT Codeplug"), "{text}");
    assert!(text.contains("DM-1701"), "{text}");
    assert!(text.contains("2021-10-26"), "{text}");
}

#[test]
fn wrap_then_unwrap_returns_the_original_segments() {
    let dir = TempDir::new();
    let a = dir.join("a.bin");
    let b = dir.join("b.bin");
    let wrapped = dir.join("fw.bin");

    let a_data: Vec<u8> = (0..0x400).map(|x| (x % 251) as u8).collect();
    let b_data: Vec<u8> = (0..0x200).map(|x| (x % 241) as u8).collect();
    std::fs::write(&a, &a_data).expect("write a");
    std::fs::write(&b, &b_data).expect("write b");

    let out = run(&[
        "wrap",
        "--radio",
        "UV3X0",
        "--segment",
        &format!("0x0800c000:{}", a.display()),
        "--segment",
        &format!("0x08040000:{}", b.display()),
        "--output",
        &wrapped.to_string_lossy(),
    ]);
    assert!(out.status.success(), "{}", combined(&out));

    let out = run(&[
        "unwrap",
        &wrapped.to_string_lossy(),
        "--output",
        &dir.join("out").to_string_lossy(),
    ]);
    assert!(out.status.success(), "{}", combined(&out));

    assert_eq!(
        std::fs::read(dir.join("out_0x0800c000")).expect("first segment"),
        a_data
    );
    assert_eq!(
        std::fs::read(dir.join("out_0x08040000")).expect("second segment"),
        b_data
    );
}

#[test]
fn wrap_writes_an_sgl_that_reads_back() {
    let dir = TempDir::new();
    let raw = dir.join("raw.bin");
    let wrapped = dir.join("fw.sgl");
    let data: Vec<u8> = (0..0x800).map(|x| (x % 253) as u8).collect();
    std::fs::write(&raw, &data).expect("write raw");

    let out = run(&[
        "wrap",
        "--radio",
        "GD77",
        "--segment",
        &format!("0x0:{}", raw.display()),
        "--output",
        &wrapped.to_string_lossy(),
    ]);
    assert!(out.status.success(), "{}", combined(&out));

    let out = run(&["fw-info", &wrapped.to_string_lossy()]);
    assert!(out.status.success(), "{}", combined(&out));
    assert!(stdout_of(&out).contains("GD77"));
}

#[test]
fn wrap_rejects_what_it_cannot_do() {
    let dir = TempDir::new();
    let raw = dir.join("raw.bin");
    std::fs::write(&raw, [0u8; 64]).expect("write raw");
    let seg = format!("0x0:{}", raw.display());
    let out_path = dir.join("out.bin");
    let out_arg = out_path.to_string_lossy().into_owned();

    // a radio nobody has heard of
    let out = run(&[
        "wrap",
        "--radio",
        "NOPE",
        "--segment",
        &seg,
        "--output",
        &out_arg,
    ]);
    assert!(!out.status.success());
    assert!(
        combined(&out).contains("unknown radio"),
        "{}",
        combined(&out)
    );

    // the SGL container has no region table, so two segments make no sense
    let out = run(&[
        "wrap",
        "--radio",
        "GD77",
        "--segment",
        &seg,
        "--segment",
        &seg,
        "--output",
        &out_arg,
    ]);
    assert!(!out.status.success());
    assert!(combined(&out).contains("one segment"), "{}", combined(&out));
}

#[test]
fn make_xor_recovers_a_key_it_can_check() {
    let file = fixture("firmware", "tyt_MD9600.bin");
    let out = run(&["make-xor", &file.to_string_lossy()]);
    assert!(out.status.success(), "{}", combined(&out));

    let text = combined(&out);
    // the fixture is tiny, so the guess will be poor and it should say so
    assert!(text.contains("not much to guess from"), "{text}");
    assert!(text.contains("known MD9600 firmware"), "{text}");
    // still prints a full key
    assert!(text.contains("03f0:"), "{text}");
}

#[test]
fn models_lists_every_container() {
    let out = run(&["models"]);
    assert!(out.status.success(), "{}", combined(&out));

    let text = stdout_of(&out);
    for expected in ["DM1701", "GD77", "CS800", "HD1", "FT70", "UV5R", "RDT"] {
        assert!(text.contains(expected), "{expected} missing from:\n{text}");
    }
}

#[test]
fn unreadable_and_unsupported_files_fail_cleanly() {
    let dir = TempDir::new();

    let out = run(&["fw-info", "/does/not/exist"]);
    assert!(!out.status.success());
    assert!(combined(&out).contains("cannot read"));

    let junk = dir.join("junk.bin");
    std::fs::write(&junk, [0x41u8; 1024]).expect("write junk");

    let out = run(&["codeplug-info", &junk.to_string_lossy()]);
    assert!(!out.status.success());
    assert!(combined(&out).contains("not supported"));
}
