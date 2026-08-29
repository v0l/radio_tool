#!/usr/bin/env python3
"""
Extract the cipher tables from the C++ headers into raw binary blobs.

The Rust port pulls these in with include_bytes!, so no table byte is ever
retyped by a human. Re-run this if a cipher header ever changes:

    python3 rust/tools/extract_ciphers.py

It prints a sha256 for each table so the output can be checked against the
C++ side independently.
"""
import hashlib
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
SRC = REPO / "include" / "radio_tool" / "fw" / "cipher"
OUT = REPO / "rust" / "crates" / "firmware" / "ciphers"

# name in the C++ header -> output file
TABLES = [
    ("dm1701", "dm1701.hpp"),
    ("md380", "md380.hpp"),
    ("md9600", "md9600.hpp"),
    ("uv3x0", "uv3x0.hpp"),
    ("sgl", "sgl.hpp"),
    ("cs800_0", "cs800.hpp"),
    ("cs800_1", "cs800.hpp"),
    ("dr5xx0", "dr5xx0.hpp"),
]

ARRAY = r"(?:const|static)\s+unsigned\s+char\s+{name}\s*\[[^\]]*\]\s*=\s*\{{(.*?)\}}\s*;"


def extract(header: str, name: str) -> bytes:
    text = (SRC / header).read_text()
    match = re.search(ARRAY.format(name=re.escape(name)), text, re.S)
    if not match:
        raise SystemExit(f"could not find array '{name}' in {header}")

    body = re.sub(r"/\*.*?\*/", "", match.group(1), flags=re.S)
    body = re.sub(r"//[^\n]*", "", body)

    values = []
    for token in body.split(","):
        token = token.strip()
        if not token:
            continue
        value = int(token, 0)
        if not 0 <= value <= 0xFF:
            raise SystemExit(f"{name}: value out of range: {token}")
        values.append(value)
    return bytes(values)


def declared_length(header: str, name: str) -> int | None:
    text = (SRC / header).read_text()
    # cs800_0 and cs800_1 share cs800_length
    stem = re.sub(r"_\d+$", "", name)
    match = re.search(rf"{re.escape(stem)}_length\s*=\s*([0-9xa-fA-F]+)", text)
    return int(match.group(1), 0) if match else None


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    failed = False

    for name, header in TABLES:
        data = extract(header, name)
        want = declared_length(header, name)
        if want is not None and want != len(data):
            print(f"MISMATCH {name}: header says {want} bytes, array holds {len(data)}")
            failed = True

        (OUT / f"{name}.bin").write_bytes(data)
        print(f"{name:10} {len(data):6} bytes  sha256={hashlib.sha256(data).hexdigest()}")

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
