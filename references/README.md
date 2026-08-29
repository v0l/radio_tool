# Reference implementations

Every claim in `rust/README.md` of the form "another project does X" was
checked against a file in here. They are committed rather than linked because
one of them disappeared from GitHub between being cited and being fetched, and
because a link tells you nothing about what the file said on the day it was
read.

All of these are unmodified copies. Licences are per file in `manifest.tsv`
and are GPL or BSD, as is radio_tool.

```
./fetch.py            check the copies match the manifest
./fetch.py --update   re-download and record what moved
```

`--update` reports `CHANGED` for anything upstream has altered since it was
recorded, which is the point: it tells you a claim below is worth rechecking.

## What each one settled

### `chirp/baofeng_uv17Pro.py`

The UV-17Pro family, which includes the UV-5R Mini. Memory regions, the 32
byte channel record, tone encoding, and the per model memory sizes all come
from here.

It also holds the trap that made a real radio look misconfigured. The
bandwidth bit is declared as `wide` and then used inverted:

```python
mem.mode = _mem.wide and self.MODES[0] or self.MODES[1]   # ["NFM", "FM"]
```

Set means narrow. Porting the struct without reading `get_memory` reports
every correctly programmed PMR446 channel as wide. Also from `get_memory`: a
channel in the airband is AM whatever that bit says.

### `chirp/uv5r.py` and `chirp/baofeng_common.py`

The classic UV-5R, which is a different radio and a different format despite
the name. Every offset, bit position, tone threshold and all 105 DTCS codes
agree with what was ported. `model_match` is why the Rust accepts idents the
C++ rejected: a real radio answers with something like `\xaaBFB231\xdd`, not
the fixed `50 bb ff` the C++ insisted on.

### `h8300-flasher/main.c`

The H8SX boot protocol, used by the Yaesu FT-70D.

**This is not an independent implementation.** It is by the same author as
radio_tool's `h8sx.cpp` and the code is the same lineage, so it corroborates
nothing about whether the protocol is right. It is here because it is the
origin of the code, and because comparing the two shows what radio_tool
already fixed:

| | h8300-flasher | radio_tool |
|---|---|---|
| `bin_sum` | uninitialised, accumulates onto a garbage value | initialised to zero |
| chunk loop | `binsize / 1024`, silently dropping the tail | rounds up, pads with `0xff` |
| sum check | four conditions joined with `&&`, so it can never fail | joined with `\|\|` |

The last of those matters more than it looks. Because the check could never
fail, the value it compares against has never actually been tested on a
radio in either project. The expected sum is accumulated as

```c
bin_sum += checksum(chunk_data, 1024);   /* checksum is a negated byte sum */
```

which is a sum of per chunk two's complement negations, not the byte sum a
Renesas User MAT sum check is documented to return. It may well be wrong.
Now that the comparison actually runs, the first person to flash an FT-70D
will find out.

### `dfu-util/dfuse.c`

The reference DfuSe host, and the thing that settled how blocks are addressed
when writing. Its download loop sets the address before **every** chunk and
always sends block number two:

```c
dfuse_special_command(dif, address, SET_ADDRESS);
/* transaction = 2 for no address offset */
ret = dfuse_dnload_chunk(dif, data + p, chunk_size, 2);
```

radio_tool's C++ instead sets the address once per sector and counts block
numbers up from two, which asks the device to compute
`pointer + (block - 2) * transfer size` for itself. A DM-1701 does not do that
arithmetic: probing its upload path shows it returns the same bytes for every
block number and ignores the address pointer entirely. Written the C++ way,
every block of an image would land at the same address.

Note the asymmetry, because it is easy to copy the wrong half: for *upload*
dfu-util does increment the transaction number, and that is exactly the
mechanism this radio ignores, which is why no useful read back is possible
from it.

### `md380tools/md380_fw.py`

TYT firmware container. The MD380 and MD2017 XOR keys are byte for byte
identical to the ones extracted here, all 1024 bytes of each, and the header
layout agrees. Its wrapped output differs from ours in 13 bytes that fall in
a region the radio ignores, and it never fills in the model name field.

### `OpenGD77/gd-77_firmware_loader.py`

SGL container. Confirms every offset: the magic as the first header key, the
second header at `0x0c`, its XOR key at `0x0e`, length at `h2+0x06`, and the
session key at `h2+0x63`. It also named the four bytes after the model key
prefix, which it sends to the radio as `encodeKey`, called `transfer_key`
here.

**The repository originally cited, `rogerclarkmelbourne/OpenGD77`, no longer
exists on GitHub.** The authors moved development off the platform. This copy
came from the `na7q` mirror. If that goes too, this file is the record.

### `dmrconfig/uv380.c`

RDT codeplug. The `0x225` header, `0x2001` timestamp, `0x2040` settings and
the whole 144 byte settings layout agree field for field. It also supplied
the CPS version field, which radio_tool did not parse.

### `ymodem/xymodem-protocol-reference.txt`

Chuck Forsberg's XMODEM/YMODEM reference. The `ymodem` module is written
against this rather than against any one implementation, which is why it
differs from `fymodem` in two places: a short final block is padded with
`0x1a` as the spec asks rather than by reading past the end of the buffer,
and a rejected block is retried a bounded number of times rather than
forever.

### `fymodem/`

The C library the C++ used for YMODEM, copied here because it is vendored
into this repository and will go when the C++ does. Kept for the same reason
as `h8300-flasher/main.c`: it is what the old behaviour was, and the two
places the Rust deliberately departs from it are only meaningful next to it.

## What has no second implementation

Ailunce. There is no other public tool that reads or writes its container, so
nothing in it has been corroborated by anyone. It is also the one container
whose cipher is not an involution, so some inputs cannot survive a round
trip. Treat it as the least trustworthy thing in the workspace.
