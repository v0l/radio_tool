#!/usr/bin/env python3
"""Minimal UV-5R clone-mode radio simulator on a PTY, for testing radio_tool."""
import os, pty, struct, sys, time

MAGIC = b"\x50\xBB\xFF\x20\x12\x07\x25"
IDENT = b"\x50\xBB\xFF\x20\x12\x07\x25"[:7] + b"\xDD"

# build a plausible radio memory
mem = bytearray(b"\xff" * 0x2000)

def bcd(freq_hz):
    v = freq_hz // 10
    out = bytearray(4)
    for i in range(4):
        out[i] = ((v // 10) % 10) << 4 | (v % 10)
        v //= 100
    return bytes(out)

chans = [
    ("SIMPLEX", 145500000, 145500000, 0, 0, 0x00),
    ("RPT1", 145625000, 145025000, 0, 0x02CD, 0x00),   # 71.9 Hz tx tone
    ("PMR1", 446006250, 446006250, 0x0017, 0x0017, 0x02),  # DTCS D023N
]
for ix, (name, rx, tx, rxt, txt, flags) in enumerate(chans):
    off = ix * 0x10
    mem[off:off+4] = bcd(rx)
    mem[off+4:off+8] = bcd(tx)
    mem[off+8:off+10] = struct.pack("<H", rxt)
    mem[off+10:off+12] = struct.pack("<H", txt)
    mem[off+12] = 0x00
    mem[off+13] = 0x00
    mem[off+14] = 0x00 if ix < 2 else 0x01     # power: high/high/low
    mem[off+15] = 0x40 | 0x04 if ix < 2 else 0x04  # wide + scan / narrow + scan
    n = 0x1000 + ix * 0x10
    mem[n:n+7] = name.encode().ljust(7, b"\xff")

mem[0x1EE0:0x1EE0+14] = b"HELLO  RADIO  "   # welcome message (radio 0x1EE0)

mem[0x1EC0+48:0x1EC0+62] = b"HN5RV01\x00\x00\x00\x00\x00\x00\x00"

master, slave = pty.openpty()
print(os.ttyname(slave), flush=True)

def read(n):
    buf = b""
    while len(buf) < n:
        b = os.read(master, n - len(buf))
        if not b:
            raise EOFError
        buf += b
    return buf

# ident handshake
got = read(len(MAGIC))
assert got == MAGIC, got
os.write(master, b"\x06")
assert read(1) == b"\x02"
os.write(master, IDENT)
assert read(1) == b"\x06"
os.write(master, b"\x06")

first = True
while True:
    try:
        cmd = read(4)
    except (EOFError, OSError):
        break
    if cmd[0:1] != b"S":
        break
    _, addr, size = struct.unpack(">BHB", cmd)
    if not first:
        os.write(master, b"\x06")
    first = False
    os.write(master, struct.pack(">BHB", ord("X"), addr, size))
    os.write(master, bytes(mem[addr:addr+size]))
    assert read(1) == b"\x06"
