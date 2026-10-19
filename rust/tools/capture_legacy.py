#!/usr/bin/env python3
"""
Record what the C++ radio_tool prints for flag invocations that need no
radio, so the Rust tool can be held to it once the C++ is gone.

    cmake -S . -B build && cmake --build build
    python3 rust/tools/capture_legacy.py

Runs from a scratch directory holding the fixtures under their own names.
Writes rust/crates/radio-tool/tests/legacy/cases.txt.
"""
import pathlib
import re
import shutil
import subprocess
import tempfile

REPO = pathlib.Path(__file__).resolve().parents[2]
CPP = REPO / "build" / "radio_tool"
OUT = REPO / "rust" / "crates" / "radio-tool" / "tests" / "legacy" / "cases.txt"
FIRMWARE = REPO / "rust" / "crates" / "firmware" / "tests" / "fixtures"
CODEPLUG = REPO / "rust" / "crates" / "codeplug" / "tests" / "fixtures"

# files the C++ misreads and the Rust reads properly, left out on purpose
SKIP = {"cs_CS800_0x100_csfwtool.bin", "chirp_baofeng_f11.img",
        "chirp_baofeng_uv5r.img", "chirp_baofeng_uv5r_mini.img"}

CASES = [
    ["--help"], ["-h", "wrap"], ["--help=Wrap"], ["--help=wrap"],
    ["--help=tyt"], ["--help=TYT Radio"], ["--help=Serial radio"], ["--help="],
    ["foo"], ["--"], ["-"], ["--", "-l"],
    ["--bogus"], ["-x"], ["-L"], ["-d=1"], ["--x"], ["-hwrap"],
    ["-lx"], ["-d", "abc"], ["-d", "-1"], ["-d", "70000"], ["-d", "1.5"],
    ["-d"], ["--dump-reg"], ["--list=x"], ["--segment"], ["--port"],
    ["-i", "x", "-o", "y"], ["--info"], ["-s", "a", "-s", "b"],
    ["--fw-info"], ["--fw-info", "-i"], ["--fw-info", "-i", "nonexistent"],
    ["--codeplug-info"], ["--codeplug-info", "-i", "nonexistent"],
    ["--unwrap", "-i", "nonexistent", "-o", "u"],
    ["--list-serial-models"],
    ["--port", "/dev/null"], ["--ble", "x"],
    ["--port", "/dev/null", "-r", "NOPE", "--info"],
    ["--ble", "x", "-r", "UV5R", "--info"],
    ["--wrap"], ["--wrap", "-o", "z"], ["--wrap", "-o", "z", "-r", "DM1701"],
    ["--wrap", "-o", "z", "-r", "NOPE", "-s", "0:none"],
    ["--wrap", "-o", "z", "-r", "DM1701", "-s", "nocolon"],
    ["--wrap", "-o", "z", "-r", "DM1701", "-s", "0x0800c000:nonexistent"],
    ["--wrap", "-o", "z", "-r", "DM1701", "-s", "zz:nonexistent"],
    ["--wrap", "-o", "z", "-r", "DM1701", "-s", "0x100000000:nonexistent"],
]


def main():
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        names = []
        for d in (FIRMWARE, CODEPLUG):
            for f in sorted(d.iterdir()):
                if f.suffix in (".bin", ".img", ".rdt") and f.name not in SKIP:
                    shutil.copy(f, tmp / f.name)
                    names.append(f.name)

        cases = list(CASES)
        for n in names:
            if n.endswith(".bin"):
                cases += [["--fw-info", "-i", n], ["--make-xor", "-i", n]]
            else:
                cases.append(["--codeplug-info", "-i", n])

        out = []
        for args in cases:
            r = subprocess.run(["radio_tool", *args], executable=CPP, cwd=tmp,
                               capture_output=True, text=True)
            err = re.sub(r"radio_tool v[0-9.]+-[0-9a-f]+", "radio_tool @VERSION@", r.stderr)
            out.append("=== " + "\x1f".join(args))
            out.append(f"rc {r.returncode}")
            out.append(f"stdout {len(r.stdout)}")
            out.append(r.stdout)
            out.append(f"stderr {len(err)}")
            out.append(err)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(out))


if __name__ == "__main__":
    main()
