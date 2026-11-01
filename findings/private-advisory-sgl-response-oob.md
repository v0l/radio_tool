# PRIVATE — submit via GitHub private vulnerability reporting on v0l/radio_tool

The security-reporting UI status could not be confirmed via API (not visible to
non-collaborators). If "Report a vulnerability" is available on the repository's
Security tab, submit the text below there and attach the patch from local branch
`audit/private-sgl-response-oob` (commit 98700e4, also in
`~/audit/patches/`). Do NOT open a public issue or PR for this.

---

## Heap out-of-bounds read from device-controlled data in TYTSGLRadio::WriteFirmware (C++ 0.5.1)

**Affected:** C++ implementation, current main (634501d), all released versions
with the SGL/DM-1701-class flash path. The Rust implementation is not affected
(it bounds-checks with `packet.get(4..4+length)`).

**Summary:** `TYTSGLRadio::WriteFirmware` validates the radio's reply to the
`DOWNLOAD` command with a three-iterator `std::equal` that walks the full
device-controlled response while reading from an 8-byte heap buffer
(`hid::tyt::commands::Update`). Any device answering `"UPDATE?"` followed by
more than 8 bytes of payload — a hostile device, or any other device sharing
the 15a2:0073 bootloader id — drives a read past the end of that buffer. The
same pattern compares the model-key response against the 8-byte
`config->header.model_key` (line 86) and, via `tyt::Command::operator==`
(include/radio_tool/hid/tyt_hid.hpp:65), runs on every `SendCommandAndOk` call
(gated there by a length equality, so only equal-length mismatches reach it).

**Evidence (reproduced under ASan, exact production expression):**

```
==349821==ERROR: AddressSanitizer: heap-buffer-overflow on address 0x7bc90efe0038
READ of size 56 at 0x7bc90efe0038 thread T0
0x7bc90efe0038 is located 0 bytes after 8-byte region [0x7bc90efe0030,0x7bc90efe0038)
SUMMARY: AddressSanitizer: heap-buffer-overflow in MemcmpInterceptorCommon
```
PoC harness: `~/audit/rt-asan/fuzz/oob_test.cpp` (replays the expression from
`src/tyt_sgl_radio.cpp:68` with a synthetic response). Reachability: the read
fires during the opening handshake of every SGL firmware flash whenever the
first 8 payload bytes equal the magic and the response carries more data; USB
DESCRIPTOR-level access is enough (no radio identification precedes it).

**Impact:** out-of-bounds heap read feeding a comparison; host crash at most
(read-only, result unused beyond the branch). Attack precondition: attacker-supplied
USB device presented as a radio. No memory write, no code execution path shown.

**CWE:** CWE-125 (out-of-bounds read) via CWE-129-precondition misuse of
`std::equal` (second range shorter than first). CERT-enumerated library
precondition violation; C++14 added the four-iterator overload as the fix.

**Proposed patch (branch `audit/private-sgl-response-oob`):**
- line 68: require `rsp.data.size() == commands::Update.size()` before comparing;
- line 86: require `rsp_key.data.size() == model_key.size()` (mismatch = reject,
  which is also the correct protocol verdict);
- optionally the four-iterator `std::equal` in `operator==`.

**Timeline for disclosure:** maintainer private contact 2026-11-01; public PR
only after fix lands or a reasonable window passes (user's call).
