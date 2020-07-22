# radio_tool

TYT/BaoFeng/(Others?) Firmware tool

# Download

For windows you can download the build artifacts from [AppVeyor](https://ci.appveyor.com/project/v0l/radio-tool)

Linux/Mac should use the build instructions below

# Building
Dependencies Linux (Ubuntu/Debian)
:
```
sudo apt install libusb-1.0-0-dev cmake gcc
```

Dependencies Mac:
```
brew install libusb cmake
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
