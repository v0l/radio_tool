#!/usr/bin/env python3
"""Fetch the reference implementations this port was checked against.

Every claim in rust/README.md about "what another project does" was checked
against one of these files. They are committed so the claims stay checkable,
and this script re-fetches them so a claim can be rechecked against whatever
upstream looks like now.

    ./fetch.py            verify the committed copies against the manifest
    ./fetch.py --update   re-download and rewrite the manifest

Everything here is GPL licensed, as is radio_tool, and every file is an
unmodified copy.
"""

import argparse
import hashlib
import pathlib
import sys
import urllib.request
from datetime import date

HERE = pathlib.Path(__file__).parent
MANIFEST = HERE / "manifest.tsv"

# where a file lives here, and where it came from
# fymodem is vendored in this repository rather than fetched, so it is not
# listed here. It is copied into references/fymodem/ because it disappears
# when the C++ does.
SOURCES = [
    (
        "chirp/baofeng_uv17Pro.py",
        "https://raw.githubusercontent.com/kk7ds/chirp/master/chirp/drivers/baofeng_uv17Pro.py",
        "GPL-3.0",
        "UV-17Pro family codeplug: memory map, channel struct, tone encoding",
    ),
    (
        "chirp/uv5r.py",
        "https://raw.githubusercontent.com/kk7ds/chirp/master/chirp/drivers/uv5r.py",
        "GPL-3.0",
        "Classic UV-5R codeplug and clone protocol, including model_match",
    ),
    (
        "chirp/baofeng_common.py",
        "https://raw.githubusercontent.com/kk7ds/chirp/master/chirp/drivers/baofeng_common.py",
        "GPL-3.0",
        "The clone handshake shared by the Baofeng drivers",
    ),
    (
        "h8300-flasher/main.c",
        "https://raw.githubusercontent.com/n1zzo/h8300-flasher/master/main.c",
        "GPL-3.0",
        "H8SX boot protocol. The ancestor of radio_tool's h8sx.cpp, not an "
        "independent implementation",
    ),
    (
        "md380tools/md380_fw.py",
        "https://raw.githubusercontent.com/travisgoodspeed/md380tools/master/md380_fw.py",
        "GPL-2.0",
        "TYT firmware container and the MD380 and MD2017 XOR keys",
    ),
    (
        "OpenGD77/gd-77_firmware_loader.py",
        "https://raw.githubusercontent.com/na7q/OpenGD77/master/Linux/FirmwareLoader/gd-77_firmware_loader.py",
        "GPL-2.0",
        "SGL container offsets and the key the radio is sent. The repository "
        "originally cited, rogerclarkmelbourne/OpenGD77, has since been taken "
        "down, so this is a mirror",
    ),
    (
        "ymodem/xymodem-protocol-reference.txt",
        "http://textfiles.com/programming/ymodem.txt",
        "public specification",
        "Chuck Forsberg's XMODEM/YMODEM reference, which the ymodem module is "
        "written against rather than against any one implementation",
    ),
    (
        "dfu-util/dfuse.c",
        "https://raw.githubusercontent.com/Stefan-Schmidt/dfu-util/master/src/dfuse.c",
        "GPL-2.0",
        "The reference DfuSe host. Settles that the address is set before "
        "every download block, with the block number always two",
    ),
    (
        "dmrconfig/dfu-libusb.c",
        "https://raw.githubusercontent.com/sergev/dmrconfig/master/dfu-libusb.c",
        "BSD-2-Clause",
        "How a codeplug is moved to and from an MD-UV380 family radio over "
        "DFU, including the block number gap and the erase addresses",
    ),
    (
        "dmrconfig/uv380.c",
        "https://raw.githubusercontent.com/sergev/dmrconfig/master/uv380.c",
        "BSD-2-Clause",
        "RDT codeplug offsets and the CPS version field",
    ),
]


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_manifest():
    if not MANIFEST.exists():
        return {}
    rows = {}
    for line in MANIFEST.read_text().splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        path, sha, fetched, licence, url, note = line.split("\t")
        rows[path] = (sha, fetched, licence, url, note)
    return rows


def write_manifest(rows):
    lines = [
        "#file\tsha256\tretrieved\tlicence\turl\twhat it settled",
    ]
    for path in sorted(rows):
        sha, fetched, licence, url, note = rows[path]
        lines.append(f"{path}\t{sha}\t{fetched}\t{licence}\t{url}\t{note}")
    MANIFEST.write_text("\n".join(lines) + "\n")


def update():
    rows = read_manifest()
    today = date.today().isoformat()
    failed = 0

    for path, url, licence, note in SOURCES:
        target = HERE / path
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            with urllib.request.urlopen(url, timeout=60) as response:
                body = response.read()
        except Exception as exc:  # noqa: BLE001
            print(f"FAILED  {path}: {exc}")
            failed += 1
            continue

        was = rows.get(path, (None,))[0]
        now = digest(body)
        target.write_bytes(body)

        if was is None:
            print(f"added   {path} ({len(body)} bytes)")
        elif was != now:
            print(f"CHANGED {path}: upstream has moved since {rows[path][1]}")
        else:
            print(f"same    {path}")

        rows[path] = (now, today if was != now else rows[path][1], licence, url, note)

    write_manifest(rows)
    return 1 if failed else 0


def verify():
    rows = read_manifest()
    if not rows:
        print("no manifest, run with --update")
        return 1

    bad = 0
    for path, (sha, fetched, _licence, _url, _note) in sorted(rows.items()):
        target = HERE / path
        if not target.exists():
            print(f"MISSING  {path}")
            bad += 1
        elif digest(target.read_bytes()) != sha:
            print(f"MODIFIED {path}: these are meant to be unmodified copies")
            bad += 1
        else:
            print(f"ok       {path} (retrieved {fetched})")

    if bad:
        print(f"\n{bad} problem(s)")
    return 1 if bad else 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--update", action="store_true", help="re-download and rewrite the manifest"
    )
    args = parser.parse_args()
    sys.exit(update() if args.update else verify())
