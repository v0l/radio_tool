//! Clone a codeplug out of the simulated radio in `test/fake_uv5r.py`.
//!
//! This is the only end to end test of the clone protocol that does not need
//! hardware. The simulator is the same one the C++ shell test uses, and the
//! image produced here has to match the one the C++ downloaded from it, which
//! is committed as a fixture in `codeplug`.
//!
//! It needs python3 and a pty, so it skips itself on Windows and when python
//! is missing.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the crate lives three levels below the repo root")
        .to_path_buf()
}

/// The simulator, and the pty it is listening on
struct Simulator {
    child: Child,
    port: String,
}

impl Simulator {
    fn start(log: &Path) -> Option<Self> {
        let script = repo_root().join("test/fake_uv5r.py");
        if !script.is_file() {
            return None;
        }

        let out = std::fs::File::create(log).expect("can create the log");
        let child = Command::new("python3")
            .arg("-u")
            .arg(&script)
            .stdout(Stdio::from(out))
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        // it prints the pty name once it is listening
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(text) = std::fs::read_to_string(log) {
                if let Some(line) = text.lines().next() {
                    if !line.trim().is_empty() {
                        return Some(Self {
                            child,
                            port: line.trim().to_owned(),
                        });
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let mut child = child;
        let _ = child.kill();
        None
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        // the simulator waits on the pty forever, so it has to be killed
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn we_clone_the_same_image_the_cpp_did() {
    if cfg!(windows) {
        eprintln!("skipping: needs a pty");
        return;
    }

    let dir = std::env::temp_dir().join(format!("radio_tool_clone_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let log = dir.join("port.txt");
    let image = dir.join("cloned.img");

    let Some(sim) = Simulator::start(&log) else {
        eprintln!("skipping: could not start the UV-5R simulator, is python3 installed?");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };

    let out = Command::new(env!("CARGO_BIN_EXE_radio-tool"))
        .args([
            "read-codeplug",
            "--port",
            &sim.port,
            "--radio",
            "UV5R",
            "--output",
            &image.to_string_lossy(),
        ])
        .output()
        .expect("the tool runs");

    let log_text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "the clone failed:\n{log_text}");
    assert!(
        log_text.contains("HN5RV01"),
        "the radio firmware version was not reported:\n{log_text}"
    );

    let cloned = std::fs::read(&image).expect("the image was written");

    // exactly what the C++ downloaded from this same simulator
    let reference = repo_root().join("rust/crates/codeplug/tests/fixtures/uv5r_hn5rv01.img");
    let expected = std::fs::read(&reference).expect("the reference image");

    assert_eq!(cloned.len(), expected.len(), "the image is the wrong size");
    assert!(
        cloned == expected,
        "the cloned image differs from the one the C++ downloaded, first at {:?}",
        cloned.iter().zip(&expected).position(|(a, b)| a != b)
    );

    drop(sim);
    let _ = std::fs::remove_dir_all(&dir);
}
