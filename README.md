# radio_tool

TYT/BaoFeng/(Others?) Firmware tool

# Download

Windows: [AppVeyor](https://ci.appveyor.com/project/v0l/radio-tool)

Linux: [Github Actions](https://github.com/v0l/radio_tool/actions)

![AppVeyor](https://ci.appveyor.com/api/projects/status/github/v0l/radio_tool?svg=true)

![Release](https://github.com/v0l/radio_tool/workflows/UbuntuRelease/badge.svg)

Otherwise you can use the instructions below to build

# Building
Dependencies Linux (Ubuntu/Debian):

```
sudo apt install libusb-1.0-0-dev cmake gcc g++ pkg-config
```

Dependencies Mac:
```
brew install libusb cmake pkg-config
```

Build:
```
git clone https://github.com/v0l/radio_tool
cd radio_tool
mkdir build && cd build
cmake ..
make -j4
./radio_tool --help
```

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
```
./radio_tool -d 0 -f -i new_firmware.bin
```

## Wrap Firmware
```
./radio_tool --wrap -o wrapped.bin -r DM1701 -s 0x0800C000:main.bin
```

## Unwrap Firmware
Output file in this case is a file prefix, the filename will be `unwrapped_0x0800C000` and others if you have
firmware will more than one segment
```
./radio_tool --unwrap -i wrapped.bin -o unwrapped 
```
