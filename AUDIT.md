# radio_tool security and privacy audit — 2026-11-01

Target: https://github.com/v0l/radio_tool at `634501d` (Release 0.5.1), GPL-3.0,
156 commits. Two implementations were audited: the C++/CMake CLI (`src/`,
`include/`, ~5.5k lines, still shipped by the Ubuntu/Windows/macOS Actions
artifacts) and the Rust workspace (`rust/`, ~20.5k lines, `unsafe_code =
"forbid"`, which is where active development happens and what the tagged
`release.yml` ships). Method: full source review of every parser and device
path, Release and ASan+UBSan builds, libFuzzer on the file parsers (two runs,
~200 execs each, seeds from the in-tree fixture corpus), targeted PoCs, git
history scan (gitleaks clean, single primary author, no force-push indicators),
CI/packaging review, and web cross-validation (libusb short-transfer semantics,
the `std::equal` two-iterator hazard, DFU/DfuSe references).

## 1. Executive summary

- **Is it safe to open untrusted firmware/codeplug files?** Mostly. No
  memory-corruption bug was found reachable purely from a file: the RDT, UV-5R,
  CS800D and all Rust parsers are bounds-checked. Two robustness defects in the
  C++ TYT container reader are fixed by PR #35 (silent truncation → zero-filled
  flash image; uncontrolled allocation from a 132-byte file, measured 4.2 GB RSS
  in the stock Release build). UBSan flags a misaligned load on every SGL parse
  (PR #36).
- **Is it safe to connect it to an untrusted device?** The C++ SGL flash path
  has one **heap out-of-bounds read driven by device-controlled bytes**
  (private advisory drafted, see `findings/private-advisory-sgl-response-oob.md`
  — not published publicly per disclosure rules), plus uninitialized-memory use
  on short USB replies (PR #37). The Rust implementation is substantially
  hardened against this class and none of these defects exist there.
- **Is it safe to flash a real radio today?** With care. Neither C++ flash path
  checks the file's model against the connected device, and 0483:df11 is the
  generic ST DFU id shared with every STM32 board in DFU mode (PR #41 adds the
  model check; the udev rule already documents the trade). The Rust CLI is the
  safer tool: dry-run by default, `--write` gate, memory-layout cross-check, and
  protocol matching. No firmware signing exists anywhere — that is a design
  limitation of the radio platforms, not a bug in this tool.
- **Privacy:** no network use anywhere (grep + code review; `test/`'s CMake
  `ExternalData` fetches hash-addressed fixtures from data.v0l.io at test
  configure time only). Codeplug files are written with default umask perms
  (0644); DMR IDs/callsigns land only in files the user names. No secrets in
  history (gitleaks clean).

## 2. Findings

| ID | Title | Severity | CWE | Status | Where | Fix |
|----|-------|----------|-----|--------|-------|-----|
| RT-01 | Device-controlled OOB read in SGL flash handshake (`std::equal` 3-arg vs 8-byte `commands::Update`; ASan: READ of size 56, 0 bytes after 8-byte region) | Medium (CVSS:3.1 6.6 local device, read-only) | CWE-125 | CONFIRMED (PoC on exact expression) | `src/tyt_sgl_radio.cpp:68,86`; `include/radio_tool/hid/tyt_hid.hpp:65` | **Private** branch `audit/private-sgl-response-oob`; advisory text in findings/ |
| RT-02 | TYT region table drives uncontrolled allocation (4.2 GB RSS / 1.7 s from a 132-byte file, exit 0) | Low (CVSS:3.1 3.3) | CWE-770 | CONFIRMED (PoC + fuzzer artifact `oom-…`) | `src/tyt_fw.cpp` Read | PR #35 |
| RT-03 | Truncated TYT firmware silently zero-filled → would flash zeros (no container checksum) | Medium (flash integrity, CVSS:3.1 6.1 user-scoped) | CWE-843/CWE-20 adjacent | CONFIRMED (PoC, exit 0, "Size: 4.00 kiB") | `src/tyt_fw.cpp` Read | PR #35 |
| RT-04 | Uninitialized stack read on short GETSTATUS/GETSTATE replies; unchecked string-descriptor indices | Low-Medium (device input) | CWE-908/CWE-457 | LIKELY (libusb permits short IN transfers; cited) | `src/dfu.cpp` GetStatus/GetState; `src/usb_radio_factory.cpp` GetDeviceString | PR #37 |
| RT-05 | Misaligned uint32/uint16 punning in SGL header (UBSan: load of misaligned address, every parse) | Low | CWE-188 | CONFIRMED (UBSan) | `src/tyt_fw_sgl.cpp:143,159,347,355,362` | PR #36 |
| RT-06 | No model check before TYT flash; 0483:df11 matches any ST DFU device | Medium (flash safety) | design + missing check | CONFIRMED (code; no hardware per rules) | `src/tyt_radio.cpp` WriteFirmware | PR #41 |
| RT-07 | `AlignedContiguousMemoryOp` silently stops at unmapped addresses → partial flash reported as "Done!" (C++) | Medium | CWE-20 (silent failure) | CONFIRMED (code) | `include/radio_tool/util/flash.hpp:134` | superseded by Rust layout check; open follow-up |
| RT-08 | UV-17Pro codeplug upload warns and proceeds on wrong-model file | Low | design | CONFIRMED (code) | `src/uv17pro_radio.cpp:328` | open follow-up (needs `--force` policy decision) |
| RT-09 | Serial port not opened exclusively (ModemManager probe desyncs clone) | Low | CWE-362 adjacent | CONFIRMED (code) | `src/serial_port.cpp` | PR #40 |
| RT-10 | `DFU::InitDownload/InitUpload` unbounded abort loop on a device stuck in an unresponsive state | Low (hang) | CWE-835 | LIKELY | `src/dfu.cpp` | already fixed in Rust (`settle()` bounds at 10); C++ follow-up |
| RT-11 | CI: default token permissions, mutable-tag action pins; artifacts unsigned (checksums provided since 0.5.x) | Low | CWE-1284 | CONFIRMED | `.github/workflows/build_*.yml` | PR #39 |
| RT-12 | Rust `dfuse::parse`: device-supplied sector count unbounded (OOM abort possible from DfuSe layout string) | Low | CWE-770 | LIKELY (code-verified via subagent sweep with line evidence) | `rust/crates/device/src/dfuse.rs:171-186` | open follow-up (cap total sectors) |
| RT-13 | Rust `main.rs` `region.address + len` u32 add unguarded (debug panic / release silent wrap) from TYT region addresses | Low | CWE-190 | LIKELY | `rust/crates/radio-tool/src/main.rs:1209,1222,1237` | open follow-up (`saturating_add` like line 1214) |
| RT-14 | C++ global mutable-ish `const` tables defined in headers (per-TU copies, ODR-adjacent); `static` header functions | Info | hygiene | CONFIRMED | `include/**` multiple | none (cleanup) |
| RT-15 | GPL-2.0-only reference sources committed in a GPL-3.0 repo (`references/`, documentation-only, licenses recorded in manifest.tsv) | Info (flag, not lawyered) | licensing | CONFIRMED | `references/manifest.tsv` | note only |

REFUTED / checked-clean: UV-5R image parser (exact size gate + bounded reads),
RDT parser (validated offsets, `ReadFixedString` never overruns), CS800D parser
(exact file-size match), Ailunce/Yaesu parsers (no header-derived indexing),
HID `operator==` OOB (length equality gates it — the earlier suspicion), UV-5R
serial clone state machine (echo validation + fixed bounds), BLE port (locked,
MTU-chunked; radio link unauthenticated is a platform design limitation to
document, not a bug), cxxopts argument handling, `--wrap -s addr:file` address
parsing (range-checked `stoull`), `--unwrap` output naming (hex-only suffix →
no traversal), make-xor histogram (bounded), BCD time math (no memory unsafety;
mktime overflow yields -1 which callers handle). Rust: clean list per the
sweep covers usb/serial/ble/hid/h8sx/ymodem/xmodem/uv5r/uv17pro/flash, all
firmware and codeplug crates (per-file notes in the session log).

## 3. Fuzzing report

Harness: `~/audit/rt-asan/fuzz/fuzz_parsers.cpp` (file-level, factory + Read +
Decrypt + segments + ToString + make-xor + Verify; ASan+UBSan+libFuzzer, malloc
cap 2 GB). Seeds: 27 in-tree fixtures. Run 1: 202 execs, OOM artifact found
(malloc(0xFFFFFFFF) — independent minimization of RT-02). Run 2 (cap 3.5 GB):
195 execs, timeout artifact (cost class of RT-02). Coverage: 917 edges/4775
PCs. Both artifacts land in the same root cause; no memory-corruption crash
was found from file input alone. Honest caveat: runs were short (~2–2.5 min);
this is a smoke pass, not a saturation campaign. Rust crates carry in-tree
`malformed_input_never_panics` fuzz-style tests; `cargo test` (workspace) and
`cargo clippy` pass clean.

## 4. Checked vs not checked

Checked-and-clean list: see above. NOT checked (residual risk): no physical
hardware (all device-path claims are code+spec evidence; the one live-device
test used the in-repo pty UV-5R simulator), no Windows/macOS runs, no real BLE
adapter, no ModemManager interaction test, fuzzing not saturated, upstream
Actions artifact contents not downloaded/inspected (login-gated).

## 5. Prioritized remediation

1. Submit the private advisory (RT-01) — text ready in
   `findings/private-advisory-sgl-response-oob.md`; hold the private branch
   until the maintainer responds or you choose to publish.
2. Merge PR #35 (RT-02/03) — closes both file-input DoS/integrity defects.
3. Merge PR #41 (RT-06) and consider a `--yes` confirmation with model + SHA256
   before erase on the C++ path (the Rust path already does the dry-run/`--write`
   equivalent).
4. Merge #36, #37, #40, #38, #39 (hardening set).
5. Rust follow-ups RT-12, RT-13 (small patches); C++ follow-up RT-07
   (report partial-flash instead of silent stop).
6. Safe-usage notes for users until then: prefer the Rust 0.5.x binaries;
   verify firmware source and hash before flashing; never flash while other
   serial/BLE tools may touch the radio; treat shared codeplugs/firmware as
   untrusted input; BLE cloning only in a trusted RF environment (link is
   unauthenticated); a mismatched-model codeplug currently only warns on UV-17Pro
   upload.

## 6. PRs opened (all draft, fork mptfire/radio_tool)

#35 region-table validation, #36 memcpy SGL header, #37 USB reply length checks,
#38 build hardening, #39 CI least-privilege + SHA pins, #40 TIOCEXCL, #41 flash
model check. Private (not pushed): `audit/private-sgl-response-oob`.

Sources: libusb control-transfer short-read semantics (libusb docs/issues #186,
#331, #366), std::equal two-iterator precondition (SO 2457072; C++14
four-iterator overload; CERT library-precondition guidance), retrieval
2026-11-01; DFU 1.1 / DfuSe behaviour from in-repo `references/dfu-util/dfuse.c`
and `references/README.md`.
