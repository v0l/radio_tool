#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::process::Command;

struct Case {
    args: Vec<String>,
    rc: i32,
    stdout: String,
    stderr: String,
}

fn take<'a>(rest: &mut &'a str, prefix: &str) -> &'a str {
    let line_end = rest.find('\n').expect("a header line");
    let header = &rest[..line_end];
    let len: usize = header
        .strip_prefix(prefix)
        .unwrap_or_else(|| panic!("expected {prefix:?}, got {header:?}"))
        .parse()
        .unwrap();
    *rest = &rest[line_end + 1..];
    let end = rest.char_indices().nth(len).map_or(rest.len(), |(i, _)| i);
    let body = &rest[..end];
    *rest = rest[end..].strip_prefix('\n').unwrap_or(&rest[end..]);
    body
}

fn cases() -> Vec<Case> {
    let text = include_str!("legacy/cases.txt");
    let mut rest = text;
    let mut out = Vec::new();
    while let Some(after) = rest.strip_prefix("=== ") {
        let end = after.find('\n').unwrap();
        let args = if end == 0 {
            Vec::new()
        } else {
            after[..end].split('\x1f').map(str::to_owned).collect()
        };
        rest = &after[end + 1..];
        let rc_end = rest.find('\n').unwrap();
        let rc = rest[..rc_end].strip_prefix("rc ").unwrap().parse().unwrap();
        rest = &rest[rc_end + 1..];
        let stdout = take(&mut rest, "stdout ").to_owned();
        let stderr = take(&mut rest, "stderr ").to_owned();
        out.push(Case {
            args,
            rc,
            stdout,
            stderr,
        });
    }
    out
}

fn fixtures(dir: &Path) {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for sub in ["firmware/tests/fixtures", "codeplug/tests/fixtures"] {
        for entry in std::fs::read_dir(crates.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
            }
        }
    }
}

#[test]
#[cfg_attr(windows, ignore = "the recording was made on Linux")]
fn the_flags_behave_as_the_cpp_tool_did() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("radio_tool_legacy_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    fixtures(&dir);

    let version = format!("radio_tool v{}", env!("CARGO_PKG_VERSION"));
    let all = cases();
    assert!(all.len() > 90, "only {} cases read", all.len());

    let mut failures = Vec::new();
    for case in &all {
        let out = Command::new(env!("CARGO_BIN_EXE_radio_tool"))
            .args(&case.args)
            .current_dir(&dir)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr)
            .replace(&version, "radio_tool @VERSION@")
            .replace(env!("CARGO_BIN_EXE_radio_tool"), "radio_tool");
        let rc = out.status.code().unwrap_or(-1);
        if rc != case.rc || stdout != case.stdout || stderr != case.stderr {
            failures.push(format!(
                "{:?}\n  rc {} want {}\n  stdout {stdout:?}\n    want {:?}\n  stderr {stderr:?}\n    want {:?}",
                case.args, rc, case.rc, case.stdout, case.stderr
            ));
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
