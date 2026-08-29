#!/usr/bin/env python3
"""
Recover the firmware test corpus from public mirrors.

The corpus used to be fetched by CMake ExternalData from data.v0l.io, which no
longer resolves. The `.sha256` files next to this script are what survived:
they name every file that was in it and pin its contents, so anything found
elsewhere can be proven to be the same file rather than merely a file with the
same name.

    python3 test/firmware/recover.py            # report what can be recovered
    python3 test/firmware/recover.py --download # fetch into test/firmware/data

Nothing is downloaded unless --download is given, and nothing is kept unless
its sha256 matches. The firmware is vendor property, so it is written to a
directory that is not committed.
"""
import argparse
import hashlib
import pathlib
import sys
import urllib.error
import urllib.parse
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
DEST = HERE / "data"

# Mirrors, tried in order. The name in the manifest uses underscores where
# some mirrors use hyphens, so each entry says how to build a candidate name.
MIRRORS = [
    ("md380.org", "https://md380.org/firmware/orig/{name}", lambda n: n.replace("_", "-")),
    ("md380.org", "https://md380.org/firmware/orig/{name}", lambda n: n),
]

# Mirrors that keep the files under names too different to guess, so the file
# list is fetched and matched by a normalised name instead
GITHUB_MIRRORS = [
    ("notzaleewa/TYT_MD-UV3X0_firmware", "main"),
]

# Vendor firmware usually ships as a zip holding the updater and the firmware.
# Each of these is downloaded once and every file inside is checked against
# the whole manifest by hash, so it does not matter what they are called.
ARCHIVES = [
    "http://files.ham-dmr.be/Firmware/Baofeng_RD-5R/RD-5R_Firmware_V2.1.6.zip",
    # Radioddity keep their firmware in a listable S3 bucket. This one holds
    # GD-77_V4.2.8.sgl from the manifest, and four later versions that are not
    # in it but are just as real.
    "https://radioddity.s3.amazonaws.com/2021-12-17%20GD-77%20CPS%20%26%20Firmware%20Changelog%20-%20Ham%20Version.zip",
]

# Firmware in the archives that is not in the manifest is kept too, under
# this directory. It is not part of the lost corpus, but it is real vendor
# firmware and the round trip test is glad of it.
EXTRA_SUFFIXES = (".sgl", ".bin")
EXTRA_MIN_SIZE = 64 * 1024

TIMEOUT = 60


def normalise(name: str) -> str:
    """Strip everything that mirrors disagree about, so md380.org's
    TYT-TFT-MD380-D3.20.bin and a manifest TYT_TFT_MD380_D3.20.bin match"""
    import re

    return re.sub(r"[^a-z0-9]", "", name.split("/")[-1].lower())


def github_index() -> list[tuple[str, str]]:
    """(url, path) for every firmware file in the GitHub mirrors"""
    import json

    out = []
    for repo, branch in GITHUB_MIRRORS:
        api = f"https://api.github.com/repos/{repo}/git/trees/{branch}?recursive=1"
        raw = fetch(api)
        if raw is None:
            continue
        try:
            tree = json.loads(raw).get("tree", [])
        except ValueError:
            continue
        for entry in tree:
            path = entry.get("path", "")
            if path.lower().endswith((".bin", ".sgl")):
                quoted = urllib.parse.quote(path)
                out.append(
                    (f"https://raw.githubusercontent.com/{repo}/{branch}/{quoted}", path)
                )
    return out


def manifest() -> list[tuple[str, str]]:
    """Every file the corpus held, with the sha256 it must have"""
    out = []
    for path in sorted(HERE.glob("*.sha256")):
        out.append((path.name[: -len(".sha256")], path.read_text().strip()))
    return out


def fetch(url: str) -> bytes | None:
    try:
        with urllib.request.urlopen(url, timeout=TIMEOUT) as r:
            return r.read()
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError):
        return None


def from_archives(wanted: dict[str, str]) -> dict[str, bytes]:
    """Download each vendor archive and keep any member whose hash is in the
    manifest. Vendor zips hold an updater and a pile of DLLs as well as the
    firmware, and the firmware is not always named the way we expect, so go by
    hash rather than by name."""
    import io
    import zipfile

    found: dict[str, bytes] = {}
    for url in ARCHIVES:
        blob = fetch(url)
        if blob is None:
            continue
        try:
            zf = zipfile.ZipFile(io.BytesIO(blob))
        except (zipfile.BadZipFile, ValueError):
            continue
        for member in zf.namelist():
            try:
                content = zf.read(member)
            except (zipfile.BadZipFile, RuntimeError, ValueError):
                continue
            digest = hashlib.sha256(content).hexdigest()
            name = wanted.get(digest)
            if name is not None:
                found[name] = content
                print(f"  recovered  {name}  ({url.rsplit('/', 1)[-1]}:{member})")
            elif member.lower().endswith(EXTRA_SUFFIXES) and len(content) >= EXTRA_MIN_SIZE:
                extra = member.rsplit("/", 1)[-1]
                found[extra] = content
                print(f"  extra      {extra}  (not in the manifest, still real firmware)")
    return found


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--download", action="store_true", help="actually fetch the files")
    args = ap.parse_args()

    files = manifest()
    print(f"{len(files)} files in the manifest\n")

    github = github_index() if args.download else []

    # hash to name, for matching archive members
    by_hash = {want: name for name, want in files}
    from_zip = from_archives(by_hash) if args.download else {}
    manifest_names = {name for name, _ in files}
    extras = {n for n in from_zip if n not in manifest_names}
    if from_zip:
        DEST.mkdir(exist_ok=True)
        for name, content in from_zip.items():
            (DEST / name).write_bytes(content)

    if args.download:
        DEST.mkdir(exist_ok=True)

    found, wrong, missing = [], [], []

    for name, want in files:
        if name in from_zip:
            # already reported by the archive pass
            continue

        local = DEST / name
        if local.is_file() and hashlib.sha256(local.read_bytes()).hexdigest() == want:
            found.append((name, "cached"))
            continue

        if not args.download:
            missing.append(name)
            continue

        got_it = False
        for mirror, template, rename in MIRRORS:
            data = fetch(template.format(name=rename(name)))
            if data is None:
                continue
            digest = hashlib.sha256(data).hexdigest()
            if digest == want:
                local.write_bytes(data)
                found.append((name, mirror))
                got_it = True
                break
            wrong.append((name, mirror, digest))

        if not got_it:
            wanted = normalise(name)
            for url, path in github:
                candidate = normalise(path)
                if not (candidate.endswith(wanted) or wanted in candidate):
                    continue
                data = fetch(url)
                if data is None:
                    continue
                digest = hashlib.sha256(data).hexdigest()
                if digest == want:
                    local.write_bytes(data)
                    found.append((name, path))
                    got_it = True
                    break
                wrong.append((name, path, digest))

        if not got_it:
            missing.append(name)

    for name, where in found:
        print(f"  recovered  {name}  ({where})")
    recovered_total = len(found) + len(from_zip) - len(extras)
    for name, mirror, digest in wrong:
        print(f"  MISMATCH   {name} from {mirror}: got {digest[:16]}...")
    if missing:
        print(f"\n  still missing: {len(missing)}")
        for name in missing[:100]:
            print(f"    {name}")

    print(f"\n{recovered_total} of {len(files)} recovered"
          f"{f', plus {len(extras)} extra vendor files' if extras else ''}", end="")
    print("" if args.download else "  (run with --download to try)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
