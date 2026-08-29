# Reference firmware files

Committed so the tests need no C++ toolchain and keep their value once the
C++ radio_tool is deleted. `manifest.tsv` lists the input each was built from:
the firmware data is regenerated in the tests from a length and a seed, so
only the wrapped output is stored.

The `source` column records which implementation wrote the file. Only
`radio_tool` files are compared byte for byte with our output, because another
project is free to differ in fields the radio ignores. Every file, whoever
wrote it, must parse back to the firmware that went in.

| source | how to regenerate |
| - | - |
| `radio_tool` | `cmake -S . -B build && cmake --build build`, then `python3 rust/tools/make_fixtures.py` |
| `md380tools` | see below |
| `csfwtool` | see below |

## Third party files

These exist because agreeing with radio_tool only proves we ported radio_tool
faithfully. It says nothing about whether radio_tool read the format
correctly. A file from a team that reverse engineered the same radios
separately is a second opinion on the format itself.

### tyt_MD380_md380tools.bin

From [md380tools](https://github.com/travisgoodspeed/md380tools):

```python
import md380_fw                       # md380_fw.py from md380tools
fw = md380_fw.MD380FW()
fw.app = sample(0x400, 5)             # the seed from manifest.tsv
open("tyt_MD380_md380tools.bin", "wb").write(fw.wrap())
```

It differs from our `tyt_MD380.bin` in 13 bytes, all of which the radio
ignores:

- `0x17..0x20`, the padding after the model name, `0xff` rather than `0x00`
- `0x7c..0x80`, the region count, left as `0xffffffff` because the field sits
  inside a run of padding md380tools never fills in

The encrypted firmware is identical. That second difference is the reason
radio_tool treats a region count of `0xffffffff` as one region, so this file
pins that rule to a real example rather than a synthetic one.

### cs_CS800_0x100_csfwtool.bin

From [CSFWTOOL](https://github.com/KG5RKI/CSFWTOOL), which is where radio_tool
got the Connect Systems format:

```sh
g++ csfwtool.cpp -o csfwtool        # needs #include <cstdint> adding first
./csfwtool -e -i plain.bin -o cs_CS800_0x100_csfwtool.bin
```

Its header and encrypted payload are byte identical to ours. It stops there,
where radio_tool appends two checksum bytes, so the file is 384 bytes against
our 386. We read both dialects and write the checksum. See the module docs in
`src/cs.rs`.
