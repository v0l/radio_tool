# radio_tool, Rust port

An in-progress rewrite of radio_tool in Rust, starting with the firmware file
formats. This is a replacement, not a second implementation to be maintained
alongside the C++: the C++ goes away when the port is complete.

## Why the formats first

The container formats are pure functions of their input, so every byte this
crate produces could be compared against the C++ implementation that has been
flashing real radios for years. The USB, HID and serial layers cannot be
validated that way without hardware, so they come later.

That comparison has already been made and its result is committed. The C++
output for every supported model is stored in `tests/fixtures`, so nothing
here depends on the C++ tool being present, or on it continuing to exist.
Deleting it will not weaken these tests.

## Layout

| path | what it holds |
| - | - |
| `crates/radio-tool` | the command line tool |
| `crates/firmware` | firmware container formats and the XOR ciphers |
| `crates/codeplug` | codeplug (channel memory) formats |
| `crates/device` | clone mode protocols, and the serial transport |
| `crates/firmware/ciphers/*.bin` | cipher tables, generated, never hand edited |
| `crates/firmware/tests/fixtures` | reference files written by the C++ tool |
| `tools/extract_ciphers.py` | regenerated those tables from the C++ headers |
| `tools/make_fixtures.py` | regenerated those reference files |

## Building and testing

```sh
cargo test
```

That is the whole story: no cmake, no libusb, no C++ binary. CI runs the same
thing plus `cargo fmt --check` and `cargo clippy -- -D warnings`.

To confirm the tests are not passing vacuously, flip a byte in one of the
cipher blobs and watch them fail, then restore it. This has been done for
every container: a broken TYT cipher byte, a broken SGL xor offset and a
broken CS checksum divisor each fail the reference tests.

### The vendor firmware corpus

The committed fixtures cover every format, but they were all written by
radio_tool or by another tool, not by the vendor. Real firmware is the better
test and cannot be committed, being vendor property and several megabytes.

The corpus that this repository used to fetch from `data.v0l.io` is gone, but
its sha256 manifest survives in `test/firmware/*.sha256`, which names every
file and pins its contents. `test/firmware/recover.py` finds what it can on
public mirrors and keeps only files whose hash matches, so a recovered file is
provably the same file rather than one with the same name:

```sh
python3 test/firmware/recover.py --download
cargo test -p firmware --test corpus
```

16 of the 58 are currently recoverable, from md380.org, a GitHub mirror, and
two vendor firmware archives. Vendor firmware ships as a zip holding an
updater and a pile of DLLs, so archive members are matched by hash rather
than by name, and firmware found in them that is not in the manifest is kept
too: it is not part of the lost corpus but it is just as real. That currently
adds four later GD-77 versions.

All 20 round trip byte for byte, across the TYT and SGL containers. Set
`RADIO_TOOL_CORPUS_REQUIRED=1` to turn a missing corpus into a failure.

### The two generator scripts

Both read the C++ and are only useful while it still exists. They are kept
for provenance, so that anyone can re-derive the committed artefacts and
satisfy themselves that nothing was transcribed by hand. Once the C++ is
deleted, delete these too: the `.bin` files become the source of truth.

```sh
cmake -S .. -B ../build && cmake --build ../build
python3 tools/extract_ciphers.py   # cipher tables
python3 tools/make_fixtures.py     # reference firmware files
```

Regenerating must be a no-op. If either changes a committed file without you
intending it, that is a bug, not something to commit.

## Cipher tables

The tables under `crates/firmware/ciphers/` are extracted mechanically
from `include/radio_tool/fw/cipher/*.hpp` so that no table byte is ever
retyped by hand:

```sh
python3 tools/extract_ciphers.py
```

The script prints a sha256 per table and refuses to write a table whose
length disagrees with the `*_length` constant in the C++ header. The output
has been checked byte for byte against the arrays as the compiler sees them.

## Status

### What has been run against a real radio

Everything in this table was done with the radio in hand, by this tool, and
the result checked. Nothing here is inferred.

| radio | over | operation | result |
|---|---|---|---|
| Baofeng DM-1701 | USB DFU | identify, memory layout, registers | works, and the registers match the C++ byte for byte |
| Baofeng DM-1701 | USB DFU | **write firmware** | works: flashed OpenRTX 0.4.4 and the radio boots |
| Baofeng DM-1701 | USB DFU | **read codeplug** | works: 851968 bytes, parses, two reads byte identical |
| Baofeng DM-1701 | USB DFU | **write codeplug** (stock firmware) | works: written and read back with no difference |
| Baofeng DM-1701 | USB DFU | write codeplug (under OpenRTX) | **impossible**, see below |
| Baofeng DM-1701 | USB DFU | **dump bootloader** | works: 48 kB, 20.86 kB used, a valid vector table. The C++ marks this macOS only |
| Baofeng DM-1701 | USB DFU | **write firmware** (stock) | works: flashed Baofeng V02.06 back over OpenRTX, 869 kB in nine sectors |
| Baofeng DM-1701 | USB DFU | **write codeplug** (stock firmware) | works: sixteen PMR446 channels and a zone, read back byte identical |
| Baofeng DM-1701 | USB DFU | **read and set the clock** (stock firmware) | works, and agrees with the C++ to the second |
| Baofeng DM-1701 | USB DFU | **read codeplug**, against dmrconfig | 851966 of 851968 bytes identical to what dmrconfig reads from the same radio. The two that differ are live state the radio changes between reads |
| Baofeng DM-1701 | USB DFU | reboot (stock firmware) | works: the radio comes back at a new bus address |
| Baofeng UV-5R Mini | Bluetooth | scan, **read codeplug** | works: 33344 bytes of memory, the size CHIRP expects, parses as sixteen PMR446 channels |
| Radioddity GD-77 | USB HID | detect in bootloader mode | works |
| Radioddity GD-77 | USB HID | write firmware | **not established**: the first bulk transfer fails with EPROTO, and the C++ fails identically at the same point, so the fault is below both tools |

### The bootloader is not the radio

Almost everything that looked like a broken protocol turned out to be this
one distinction, so it is worth stating plainly. A TYT radio speaks DFU in
two quite different ways:

- **its own firmware**, when the radio is switched on normally. This is what
  implements the `0x91` vendor commands: the codeplug, the clock, reboot.
  dmrconfig talks to this, and so does the stock CPS.
- **its bootloader**, entered by holding PTT and the button above it. This
  flashes firmware and nothing else.

With OpenRTX installed there is no firmware DFU at all, only a serial port,
so the bootloader is all that is left. Everything vendor specific then fails
in the same quiet way: codeplug writes accepted and discarded, erases
accepted and discarded, reboot ignored, reads above 0x40000 answering `0xff`,
and the clock register answering the same four bytes as another register.
Each looked like success or like empty memory.

The evidence for all of it, and the corrections it forced:

| under the bootloader | under stock firmware |
|---|---|
| codeplug reads return `0xff` above 0x40000, so the radio looks like it has no channels | the same read returns a full DMR codeplug, 506796 bytes different |
| codeplug writes are accepted and stored nowhere | writes work and read back byte identical |
| the clock register holds `4b 01 00 10`, which is not a time | it holds a real timestamp and can be set |
| register 2 and the clock register return the same bytes | they differ, as they should |
| reboot is ignored | the radio restarts and comes back at a new address |
| `usb-info` reports internal flash at 0x0800c000, 976 kB | the same radio reports 48 kB at 0x08000000, a 128 MB flash, and 1 MB of SPI |

That last row is the argument for reading the memory layout from the device
rather than compiling in a chip map: the same radio describes itself
differently depending on what is running.

`dfu::codeplug::write` reads back a sample of what it wrote and fails rather
than reporting success, which is what catches the bootloader case.

### What a UV-5R Mini answers over Bluetooth

Its clone protocol addresses sixteen bits, so 64 kB in total, and all of it
reads cleanly: a sweep of the whole space returned every block. Deciphering
the known regions with key 1 reproduces the codeplug byte for byte, which is
an independent check on both the cipher and the block handling.

Three regions answer that nothing reads. CHIRP's `MEM_STARTS` for this radio
is `[0x0000, 0x9000, 0xA000]` and radio_tool follows it, so these are outside
what either tool touches:

```
0xb000  355 of 1024 bytes used
0xd000   30 of 1024 bytes used
0xf000  640 of 1024 bytes used, repeating four byte groups
```

The content at 0xf000 looks like per band calibration, but that is a guess
from its shape and has not been confirmed.

There is no firmware here. The program flash is not in this address space and
the protocol has no command that reaches it, so firmware cannot be read out
of one of these radios over Bluetooth.

Connecting is unreliable: three attempts were needed, failing with
`le-connection-abort-by-local` and a service discovery timeout before one
succeeded. That is BlueZ rather than the radio, and anything doing real work
over this link wants a retry loop.

### What is implemented and tested, but has not met hardware

These are exercised against in memory fakes, and in one case against a pty
simulator, but no radio has ever answered them. Treat them as unproven.

- Writing a codeplug to a UV-17Pro family radio, including over Bluetooth
- The TYT HID protocol for SGL radios, checked field by field against
  OpenRTX's own loader and against the C++
- YMODEM, for the Ailunce HD1
- XMODEM, for reading flash out of a radio running OpenRTX. The protocol is
  right, but OpenRTX 0.4.4 defines `eflash_dump` and `eflash_restore` and
  calls them from nowhere, so there is no way to start a transfer
- H8SX, for the Yaesu FT-70D, including a sum check that has never once run
- Reading a codeplug from a classic UV-5R over a cable, which is checked
  against `test/fake_uv5r.py` and matches the C++ byte for byte, but the
  simulator is not a radio

### What is done and needs no radio

- XOR cipher tables and `apply_xor`
- TYT container (MD-380 family, MD-9600, MD-UV3x0, MD-2017, DM-1701):
  parse, serialise, segments, model table, alignment, compatibility
- SGL container (GD-77, GD-77S, BF-5R, DM-1801): both headers, the rotate
  and invert obfuscation, header secrets
- Connect Systems container (CS800, CS800D) including the checksum
- Ailunce HD1 and Yaesu FT-70D, both headerless
- Reference tests against C++ written files for 10 TYT models, 4 SGL models,
  3 CS sizes and 3 headerless files, all committed so they outlive the C++
- Twenty real vendor firmware files, round tripped byte for byte
- UV-5R codeplug, UV-17Pro family codeplug, RDT codeplug
- XOR key recovery for a radio nobody has looked at yet
- The command line tool, and CI

### Working with a codeplug

This tool does not edit codeplugs and should not. Channels, zones, contacts
and the rest are a CPS's job, and dmrconfig does it properly for this family.
It reads an image straight out of `usb-read-codeplug`, prints an editable
text configuration, and applies one back to an image offline. dmrconfig
decides what a codeplug should contain; this moves it to and from the radio.

```
radio-tool usb-read-codeplug --output dm1701.img
dmrconfig dm1701.img > dm1701.conf
$EDITOR dm1701.conf
dmrconfig -c dm1701.img dm1701.conf
radio-tool usb-write-codeplug --input device.img --write
```

Two things to know before doing that on a DM-1701.

Its contact records begin at image offset `0x70014`, and dmrconfig looks for
them at `0x70000`, so it finds none and every digital channel fails to
resolve its talkgroup. The records themselves are fine, with a correct 36
byte stride from there. This is in dmrconfig's own image too, so it is its
RT84 support inheriting MD-UV380 offsets rather than anything wrong with the
transfer.

A zone has an A list and a B list, and they live in different places: the A
channels are in the zone record at `0x149e0` and the B channels are in the
zone extension at `0x31000`. Fill in only one and the radio shows
"Unprogram" on the other VFO.

### Parity with the C++

Every operation the C++ offers has an equivalent here, though several are
named differently: `dump-reg` and `get-status` are covered by `usb-registers`,
`info` by `usb-info`, `list` by `usb-list`, `list-ble` by `ble-scan`,
`list-serial-models` by `models`, and `program` by `write-codeplug` and
`usb-write-codeplug`.

Two things the C++ does are deliberately not carried over as they stand:

- `set-time` writes a clock into a register that, on the only radio available
  to test with, is not a clock. `usb-time` reads it and says so rather than
  printing a date derived from bytes that are not a timestamp. The C++ prints
  `RTC: Sun Dec  9 23:00:00 5100` from those same four bytes.
- `reboot` is here, but on a radio in bootloader mode it does nothing: the
  `0x91` vendor commands are implemented by the radio's own firmware, not by
  the bootloader. The command checks whether the radio actually left the bus
  and says so rather than reporting success.
- The command line cannot flash an Ailunce HD1 or a Yaesu FT-70D. Both
  protocols are implemented and tested against fakes, but wiring them up
  would ship a command that can brick a radio nobody here can test on.

This tool does not build codeplugs, and should not. Editing channels, zones,
contacts and the rest is what a CPS is for, and dmrconfig does it properly
for this family: it reads an image straight from `usb-read-codeplug`, prints
an editable text configuration, and applies one back to an image offline. The
division is that dmrconfig decides what a codeplug should contain, and this
moves it to and from the radio.

```
radio-tool usb-read-codeplug --output dm1701.img
dmrconfig dm1701.img > dm1701.conf
$EDITOR dm1701.conf
dmrconfig -c dm1701.img dm1701.conf
radio-tool usb-write-codeplug --input device.img --write
```

Things here that the C++ does not do at all: reading a DM-1701 codeplug,
which it refuses outright; reading the radio's own memory layout so a flash
cannot erase the bootloader; verifying that a codeplug write was actually
stored; `make-pmr`; and udev rules, without which none of the USB radios can
be reached without root.

### Not started

- Writing a codeplug to anything but the UV-17Pro family

## The device layer, and how far it can be trusted

Everything else in this workspace can be checked against a file somebody else
produced. A protocol cannot: a mistake only shows when it is talking to a
radio, and a radio confused mid clone is not always easy to recover.

So the protocols are written against a `ByteStream` trait rather than against
a serial port, which makes the whole conversation testable. There are two
levels of check:

- an in memory fake radio that answers the clone protocol from an image and
  records anything the driver does that a real radio would not expect, such
  as reading past the end of memory or sending an unknown command
- the pty simulator in `test/fake_uv5r.py`, driven through the real serial
  transport, with the resulting image compared against the one the C++ tool
  downloaded from that same simulator. They match byte for byte.

That is enough to say the reading side behaves the way the C++ does. It is
not enough to say either is right about a real radio, which is why writing is
not implemented.

### YMODEM

The Ailunce HD1 takes firmware over YMODEM, which the C++ handed to the
`fymodem` C library. Rewriting it rather than binding to it was worth the
effort for two reasons found on the way:

- `fymodem` always transmits a full 1024 byte block, so a final block shorter
  than that reads past the end of the caller's buffer and sends whatever was
  next in memory. A receiver truncates to the declared size, so the file still
  arrives, but reading out of bounds is not something to keep.
- It resends a rejected block forever. A receiver that keeps saying no hangs
  the transfer with nothing to do but kill the process.

Both are covered by tests, and the retry limit is checked by a fake receiver
that refuses everything and fails the test rather than letting it hang.

Because YMODEM is a published standard rather than something recovered from a
vendor tool, the tests can check it against the spec: the CRC has a documented
check value, and the transfer is exercised across block boundaries, past the
point where block numbers wrap at 256, and with a receiver that cancels.

### USB

`nusb` rather than libusb, so there is no C library to find and nothing new
for CI to install. The crate splits into two halves on purpose:

- Which radio a USB device is, is a lookup in `usb::KNOWN` with no hardware
  involved, so it is tested. Including that nothing else is claimed: on the
  machine this was written on it correctly passed over eight devices, among
  them a security key and a Bluetooth adapter.
- Opening the device and moving bytes is a thin wrapper that cannot be tested
  without a radio, so it holds no decisions at all.

The identifiers belong to bootloaders rather than to radios. `0483:df11` is
the ST DFU bootloader, which is on a great many devices that are not radios,
so `usb-list` reports the mode it saw and does not promise a model. Asking
the radio, with `usb-info`, is what gets a model, and it only reads.

### A radio is sent the stored form, not the image

`TYTFW::Read` in the C++ does not decipher. Only `--unwrap` asks for that.
`WriteFirmware` sends the payload exactly as it sits in the file, still
enciphered, and the bootloader deciphers it itself.

This port deciphered on parse, so `segments()` returned the plain image and
the first two flashes of a DM-1701 sent that. The radio took all 223 blocks,
acknowledged every one, reported success, and would not start. Twice.

```
file at data offset 0x100: 3a 06 74 8d 95 5b b2 70   what the radio wants
deciphered image         : 00 02 00 20 05 f5 01 08   what was sent
```

The second line is a valid ARM vector table, stack pointer then reset
handler, which is why checking it looked like confirmation the image was
right. It was the evidence it was wrong.

`TytFirmware::segments_as_stored` returns the stored form and is what the
flash path uses. Three tests hold it: it is byte for byte the data section of
the file, it differs from the deciphered form, and deciphering it gives the
image back.

**On block addressing, which is not settled.** Chasing this failure through
the USB layer first, the write loop was changed from the C++ arrangement,
address set once per sector with block numbers counting up from two, to what
dfu-util does for a DfuSe device, address before every block with the number
always two. That did not fix it, because the payload was the problem. The
code is back to the C++ arrangement, which is now confirmed working on a
DM-1701 with the correct payload. The dfu-util arrangement was never tried
with a correct payload, so it is untested here rather than disproven, and
there is no reason to prefer it: the C++ was reverse engineered from the
vendor software and works.

### Reading a TYT radio back

There is no read back from these radios. Upload ignores addressing, and after
any download it answers with the buffer of the last command it was sent, so a
read after setting an address returns `21` followed by that address. There is
no verification after a flash and no restorable backup. `usb-dump` detects
both cases and refuses rather than writing a file that looks like a backup.

### The radio knows its own memory better than we do

An ST DFU device describes its memory in the interface string of each
alternate setting, in the format ST documents in AN3156. A DM-1701 says:

```
@Internal Flash   /0x0800C000/01*016Kg,01*064Kg,07*128Kg
@SPI Flash Memory /0x00000000/16*064Kg
```

Read that carefully. Internal flash starts at `0x0800C000`, not at
`0x08000000` where the chip's flash begins. The three 16K sectors below it
hold the bootloader, and the radio is deliberately not offering them.

The C++ erases using a hardcoded STM32F40x map that covers the whole chip
from `0x08000000`, so it will happily erase those sectors on a radio that is
trying to say no. `dfuse::parse` reads what the radio actually offers, and
`dfu::flashing::write` refuses any image that falls outside it before sending
the radio a single byte. There is a test for exactly this, using the layout
string above as it came off the radio: the same image is refused against the
radio's map and accepted against the chip map.

`flash::STM32F40X` is kept only as a last resort for a device that will not
describe itself. Prefer the device.

### DFU, and a loop that could not end

The TYT radios take firmware over USB DFU, with a few vendor commands on top.
DFU is control transfers rather than a stream, so it has its own narrow
trait, `dfu::ControlTransfer`, for the same reason `ByteStream` exists: the
conversation can then be run against a fake and checked.

Getting a device ready for a transfer was a `while (1)` calling abort until
the device said it was idle. Abort cannot clear the DFU error state, that is
what the clear status request is for, so a radio sitting in that state hung
the tool with nothing to do but kill it. Here the error state is cleared with
the request meant for it, and the loop is bounded.

The fake device counts requests and fails the test if the host makes too
many, so an unbounded loop shows up as a failure rather than a run that never
finishes.

### H8SX, and a check that has never run

The Yaesu FT-70D takes firmware over the H8SX boot protocol. That code came
to radio_tool from `h8300-flasher`, which is by the same author, so the two
are one lineage rather than two implementations that agree. Both are in
`references/`.

Its final step asks the device for a sum over what was written. In the
original the comparison joined four conditions with `&&`, so it could never
fail, which means the expected value has never been tested against a radio in
either project. That value is a sum of per block two's complement negations,
which is not the byte sum a Renesas user MAT sum check is documented to
return.

Rather than guess, `h8sx::flash` computes both candidates and reports them
next to what the device said:

```
the device reported 0x0004b1c8, which matches neither a byte sum
(0x0004b1c8) nor a sum of block checksums (0x00000fa0)
```

One flash on real hardware settles it. Until then the protocol is implemented
and tested against a fake device, and is not reachable from the command line.

### What a real radio has confirmed

A Baofeng UV-5R Mini, over Bluetooth, on the second attempt at a connection:

- it advertises as `walkie-talkie`, with no other clue as to what it is
- it puts its serial link on the HM-10 service, writing and notifying on the
  same characteristic, which is the case the service selection prefers
- the download is 33344 bytes of memory, exactly the size CHIRP's own UV-5R
  Mini image is once its metadata trailer is removed
- the obfuscation key is right: channel names come back as readable text and
  the frequencies decode as valid PMR446 channels

The image it produced then parses with the codeplug reader in this workspace,
showing sixteen PMR446 channels at the correct 12.5 kHz spacing with their
names and CTCSS tones, which closes the loop from Bluetooth to displayed
channel list.

That radio also caught a bug. CHIRP's channel struct has a bit named `wide`
which it then uses inverted, so set means narrow, and reading the struct at
face value reported a set of correctly programmed PMR446 channels as wide
when they are narrow. Anyone porting from a CHIRP struct definition should
check how each field is used and not only how it is declared. That radio's codeplug is not committed: it is somebody's actual
channel memory. CHIRP's own UV-5R Mini test image is, and covers the same
ground.

So the Bluetooth transport, the UV-17Pro protocol and the obfuscation are
confirmed against hardware. The serial transport is not: nothing in this
family has been read over a cable yet, and the classic UV-5R has no
Bluetooth, so its driver is still only checked against the simulator.

The UV-17Pro obfuscation was additionally compared against the C++ over all
256 byte values, which is the whole input space of that function, so that one
is settled.

## Porting notes

Things in the C++ worth knowing before porting the rest, all of which this
port either fixed or deliberately kept:

- `TYTFW::Create()` uses the default constructor, so segment alignment is 0
  in the only path the CLI uses. The 0x200 alignment in the other constructor
  is dead. `append_segment` here takes the alignment as an argument.
- A region count of `0xffffffff` in the header means one region.
- The counter magic is length prefixed: the first byte is the number of bytes
  that follow it.
- The SGL header stores the binary length, and the CLI sets the radio model
  before appending any segment, so the length has to be fixed up at encrypt
  time. Do not repeat the bug where it is captured as zero. This port sets
  the length at serialise time from the data it actually holds.
- The C++ reads the SGL version starting one byte into the three digit
  field, so "ENCV100" parses as 0 rather than 100. Only version 1 exists, so
  it has never mattered. This port reads all three digits.
- SGL header secrets (binary offset, header 2 offset and its XOR key, the
  model key filler) came from a default seeded `std::default_random_engine`
  in the C++, so every file it wrote carried identical "random" values. This
  port seeds from the OS, which means SGL output cannot be compared byte for
  byte between the two tools. The differential tests check interop in both
  directions instead.

- MD380 and MD446 share a counter magic, so an MD446 file reads back as an
  MD380. The C++ does the same. It is a property of the format, not a bug.
- The Ailunce obfuscation is not reversible for every input, and the C++ uses
  one function for both directions as though it were. Whole words are safe
  except `0x07777777` and `0xfeeeeeee`; trailing bytes, when the length is
  not a multiple of four, are safe only for `0x00` and `0xff`. Both verified
  against the C++ by wrapping twice. Real firmware is word aligned and mostly
  padding, which is why it has never bitten. Ported as is, because the radio
  defines the format. `ailunce::lossy_offsets` reports affected bytes.
- `YaesuFW::SupportsRadioModel` returned true for every model in the C++, so
  it claimed `HD1` before the Ailunce handler could and wrote Ailunce
  firmware out unencrypted. Fixed in the C++ and not reproduced here.

## Cross references

Each of these formats was reverse engineered by more than one team. Comparing
against the others is the only way to tell a faithful port of radio_tool from
a faithful reading of the format, and it turned up things worth knowing.

| format | other implementation | result |
| - | - | - |
Every file cited below is committed under `references/`, with a manifest
recording where it came from, when, and its licence. `references/fetch.py`
re-downloads them and reports what has moved upstream. That is not
belt and braces: the OpenGD77 repository cited here was taken down between
being read and being archived, and only the mirrored copy is left.

| TYT | [md380tools](https://github.com/travisgoodspeed/md380tools) `md380_fw.py` | MD380 and MD2017 XOR keys identical to ours, all 1024 bytes each. Header layout agrees. Wrapped output differs in 13 bytes the radio ignores. We parse its files. |
| SGL | [OpenGD77](https://github.com/rogerclarkmelbourne/OpenGD77) `gd-77_firmware_loader.py` | Every offset agrees: magic as the header 1 key, header 2 offset at 0x0c, its XOR key at 0x0e, length at h2+0x06, session key at h2+0x63. |
| Connect Systems | [CSFWTOOL](https://github.com/KG5RKI/CSFWTOOL) | Both ciphers identical to ours. Header struct identical. Writes no trailing checksum, where radio_tool writes and requires one. |
| Ailunce | none found | radio_tool appears to be the only implementation. |
| UV-5R codeplug | [CHIRP](https://github.com/kk7ds/chirp) `drivers/uv5r.py` | Every offset, bit position, tone threshold and all 105 DTCS codes agree. |
| RDT codeplug | [dmrconfig](https://github.com/sergev/dmrconfig) `uv380.c` | 0x225 header, 0x2001 timestamp, 0x2040 settings and the whole 144 byte settings layout agree field for field. |
| TYT firmware | 14 real vendor files | All round trip byte for byte, after correcting the model name padding and preserving the region count as written. |
| SGL firmware | 6 real vendor files | All round trip byte for byte, after preserving the header filler. |
| UV-5R codeplug | 2 real images from CHIRP's test suite | Both parse. Neither carries the ident the C++ requires, and the C++ rejects both. |

What came out of it:

- **The keys are confirmed by independent work.** Two teams derived the same
  1024 byte MD380 and MD2017 keys, and two the same 256 byte CS800 and DR5XX0
  keys. These are not transcription artefacts of one project.
- **`0xffffffff` as a region count is a real case, not a curiosity.**
  md380tools never fills that field in, because it falls inside a run of
  padding. Any file it produced needs the special case radio_tool has.
- **The four bytes after the SGL model key prefix are not padding.** A flasher
  reads them out of the file and sends them to the radio to open the session.
  OpenGD77 calls this the encodeKey. Named `transfer_key` here, where the C++
  treated them as random filler.
- **The Connect Systems checksum is unsettled.** CSFWTOOL does not write one
  and radio_tool insists on one, and radio_tool's own source says "checksum
  not working right now". radio_tool is probably right, since its size check
  would otherwise reject every vendor file, but nobody has confirmed it
  against a radio. This port writes the checksum and reads files either way.
- **The Ailunce format has no second opinion available.** It arrived in
  radio_tool in a single 2022 commit and no other public implementation was
  found, so the two flaws in its obfuscation, described in `src/ailunce.rs`,
  are unverifiable against anything but that one source. Treat that container
  as the least trustworthy of the five.
- **The UV-5R codeplug came through clean.** CHIRP is the reference for this
  family and is exercised by far more radios than this crate ever will be.
  Its channel offsets, the MSB first bit positions in `MEM_FORMAT`, the
  `0x0258` CTCSS threshold, the `0x6a` inverted DTCS base and the whole
  `sorted(DTCS_CODES + (645,))` table all match what radio_tool had, so the
  port follows both.
- **Vendor firmware fills the SGL header with bytes nobody has explained.**
  Header 1 is reproduced exactly from the fields we understand, but the space
  around it and around header 2, out to the start of the firmware, holds data
  that both radio_tool and this port used to replace with zeroes. It is kept
  now, so reading a file and writing it back changes nothing. Whether the
  radio reads any of it is unknown, which is the reason to preserve it.
- **radio_tool does not write the TYT model name field the way the vendor
  does,** and this port now follows the vendor. Genuine firmware writes the
  model, two nulls, then `0xff` to the end of the 16 byte field. md380tools
  does the same. radio_tool zero fills. With that fixed, and with the region
  count preserved as written rather than normalised, all 14 recovered vendor
  files round trip byte for byte, where before none did. Whether the radio
  cares is unknown; matching real firmware is the safer side to be on.
- **The RDT codeplug came through clean too,** and gained a field. dmrconfig
  agrees on the 0x225 header, the 0x2001 timestamp and the 0x2040 settings
  block, and its `general_settings_t` matches radio_tool's reader field for
  field including every run of skipped bytes. It also decodes four bytes
  after the timestamp as the CPS software version, which radio_tool skips, so
  this port reads them. An unwritten version field reads as absent rather
  than as V00.00.
- **RDT timestamps are validated rather than trusted.** The C++ fed the BCD
  straight to `mktime`, which is why it needed a guard against `ctime`
  returning null. This port rejects anything that is not valid BCD or not a
  real date, and says so.

## Reference testing, and its limits

A symmetric mistake in a cipher cancels itself out over a round trip: encrypt
wrongly, decrypt wrongly, and the original bytes come back. A round trip test
therefore proves nothing about a cipher. What proves it is a *fixed* file
that something else produced, which is what `tests/fixtures` holds.

When adding a container: capture fixtures from the C++ first, assert both
that you parse them to the right firmware and that your writer reproduces
them byte for byte, then prove the assertions bite by breaking a cipher byte
on purpose.

For containers whose header carries values the writer picks at random, SGL
being the one so far, byte comparison against a fresh write is impossible.
Parse the reference file and write it back instead: that must reproduce the
original exactly, which pins the header writer just as tightly.
