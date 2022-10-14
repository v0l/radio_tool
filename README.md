# radio_tool

Radio Firmware tool

## Radio Support

| Manufacturer | Radio Model | Fw Read | Fw Write | Fw Wrap | Db Read | Db Write |
| - | - | - | - | - | - | - |
| TYT | [MD-2017](https://www.tyt888.com/?mod=product_show&id=110)| ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| TYT | [MD-9600](https://www.tyt888.com/?mod=product_show&id=108) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| TYT | [MD-UV380](https://www.tyt888.com/?mod=product_show&id=127) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| TYT | [MD-UV390](https://www.tyt888.com/?mod=product_show&id=129) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| TYT | [MD-390](https://www.tyt888.com/?mod=product_show&id=77) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| TYT | [MD-380](https://www.tyt888.com/?mod=product_show&id=78) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| TYT | [MD-446](https://www.tyt888.com/?mod=product_show&id=75) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| TYT | [MD-280](https://www.tyt888.com/?mod=product_show&id=80) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| Baofeng | [DM-1701](https://www.baofengradio.com/products/dm-1701) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| Connect Systems | [CS800D](https://www.connectsystems.com/products/top/radios/CS800D.htm) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| Ailunce | [HD1](https://www.ailunce.com/Product/HD1/Overview) | ✖️ | ✔️ | ✔️ | ✖️ | ✖️ |
| Yaesu | [FT-70DR](https://www.yaesu.com/indexVS.cfm?cmd=DisplayProducts&ProdCatID=249&encProdID=7CDB93B02164B1FB036530FBD7D37F1A&DivisionID=65&isArchived=0) | ✖️ | ✔️ | ✖️ | ✖️ | ✖️ |

```
Fw = Firmware
Db = Codeplug database
```
# Download

Windows: [AppVeyor](https://ci.appveyor.com/project/v0l/radio-tool)

Linux: [Github Actions](https://github.com/v0l/radio_tool/actions)

![AppVeyor](https://ci.appveyor.com/api/projects/status/github/v0l/radio_tool?svg=true)

![Release](https://github.com/v0l/radio_tool/workflows/UbuntuRelease/badge.svg)

Otherwise you can use the instructions below to build

# Building
Dependencies Linux (Ubuntu/Debian):

```bash
sudo apt install libusb-1.0-0-dev cmake gcc g++ pkg-config
```

Dependencies Mac:
```bash
brew install libusb cmake pkg-config
```

Build:
```bash
git clone https://github.com/v0l/radio_tool
cd radio_tool
mkdir build && cd build
cmake ..
make -j4
./radio_tool --help
```

# Docs
Code documentation: https://data.v0l.io/radio_tool/docs

# Usage
```
Usage:
  ./radio_tool [OPTION...]

 General options:
  -h, --help <command>  Show this message
  -l, --list            List devices
  -d, --device <index>  Device to use
  -i, --in <file>       Input file
  -o, --out <file>      Output file
  -L, --list-radios     List supported radios

 Programming options:
  -f, --flash    Flash firmware
  -p, --program  Upload codeplug

 Firmware options:
      --fw-info  Print info about a firmware file
      --wrap     Wrap a firmware bin (use --help wrap, for more info)
      --unwrap   Unwrap a fimrware file

 All radio options:
      --info                 Print some info about the radio
      --write-custom <data>  Send custom command to radio
      --get-status           Print the current DFU Status

 TYT Radio options:
      --get-time             Gets the radio time
      --set-time             Sets the radio time
      --dump-reg <register>  Dump a register from the radio
      --reboot               Reboot the radio
      --dump-bootloader      Dump bootloader (Mac only)

 Codeplug options:
      --codeplug-info  Print info about a codeplug file
```

## Flash Firmware
```bash
./radio_tool -d 0 -f -i new_firmware.bin
```

## Wrap Firmware
```bash
./radio_tool --wrap -o wrapped.bin -r DM1701 -s 0x0800C000:main.bin
```

## Unwrap Firmware
Output file in this case is a file prefix, the filename will be `unwrapped_0x0800C000` and others if you have
firmware will more than one segment
```bash
./radio_tool --unwrap -i wrapped.bin -o unwrapped 
```
