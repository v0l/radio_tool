#!/usr/bin/env python3
"""
Capture reference firmware files produced by the C++ radio_tool.

Run once, while the C++ tool still exists. The files it writes are committed
as test fixtures, so the Rust tests keep checking against the behaviour that
has been flashing real radios even after the C++ is deleted.

    cmake -S . -B build && cmake --build build
    python3 rust/tools/make_fixtures.py

Every fixture is regenerated from scratch, so re-running this after a
deliberate format change and committing the result is how you accept that
change. If a fixture moves without you intending it, that is a bug.
"""
import pathlib
import shutil
import subprocess
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parents[2]
OUT = REPO / "rust" / "crates" / "firmware" / "tests" / "fixtures"

# kind, model, address, length, seed
CASES = [
    ("tyt", "DM1701", 0x0800C000, 0x400, 1),
    ("tyt", "MD9600", 0x0800C000, 0x400, 2),
    ("tyt", "UV3X0", 0x0800C000, 0x400, 3),
    ("tyt", "UV3X0 GPS", 0x0800C000, 0x400, 4),
    ("tyt", "MD380", 0x0800C000, 0x400, 5),
    ("tyt", "MD390", 0x0800C000, 0x400, 6),
    ("tyt", "MD446", 0x0800C000, 0x400, 7),
    ("tyt", "MD280", 0x0800C000, 0x400, 8),
    ("tyt", "MD2017", 0x0800C000, 0x400, 9),
    ("tyt", "MD2017 GPS", 0x0800C000, 0x400, 10),
    ("sgl", "GD77", 0x0, 0x400, 11),
    ("sgl", "GD77S", 0x0, 0x400, 12),
    ("sgl", "BF5R", 0x0, 0x400, 13),
    ("sgl", "DM1801", 0x0, 0x400, 14),
    # sizes either side of the 256 byte cipher period, where the checksum mask
    # is picked by the image size
    ("cs", "CS800", 0x20000, 0x100, 15),
    ("cs", "CS800", 0x20000, 0x101, 16),
    ("cs", "CS800", 0x20000, 0x2001, 17),
    # headerless containers. The 0x102 case has a two byte tail, which the
    # Ailunce obfuscation mangles in a way worth pinning down
    ("ailunce", "HD1", 0x0, 0x400, 18),
    ("ailunce", "HD1", 0x0, 0x102, 19),
    ("yaesu", "FT70", 0x0, 0x400, 20),
]


def sample(length: int, seed: int) -> bytes:
    """The xorshift the Rust tests use, so fixtures need no input files."""
    x = seed | 1
    out = bytearray()
    for _ in range(length):
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        x &= 0xFFFFFFFF
        out.append(x & 0xFF)
    return bytes(out)


def slug(kind: str, model: str, length: int) -> str:
    name = model.replace(" ", "_")
    if kind in ("cs", "ailunce"):
        return f"{kind}_{name}_{length:#x}.bin"
    return f"{kind}_{name}.bin"


def main() -> int:
    tool = REPO / "build" / "radio_tool"
    if not tool.is_file():
        print(f"the C++ radio_tool is not built at {tool}", file=sys.stderr)
        print("run: cmake -S . -B build && cmake --build build", file=sys.stderr)
        return 1

    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True)

    manifest = ["#kind\tmodel\taddress\tlength\tseed\tfile"]

    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        for kind, model, address, length, seed in CASES:
            segment = tmp / "segment.bin"
            segment.write_bytes(sample(length, seed))

            name = slug(kind, model, length)
            wrapped = OUT / name

            result = subprocess.run(
                [
                    str(tool),
                    "--wrap",
                    "-r",
                    model,
                    "-s",
                    f"{address:#x}:{segment}",
                    "-o",
                    str(wrapped),
                ],
                capture_output=True,
                text=True,
            )
            if result.returncode != 0 or not wrapped.is_file():
                print(f"failed to wrap {model}: {result.stdout}{result.stderr}", file=sys.stderr)
                return 1

            manifest.append(f"{kind}\t{model}\t{address:#x}\t{length:#x}\t{seed}\t{name}")
            print(f"{name:28} {wrapped.stat().st_size:7} bytes")

    (OUT / "manifest.tsv").write_text("\n".join(manifest) + "\n")
    print(f"\nwrote {len(CASES)} fixtures to {OUT.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
