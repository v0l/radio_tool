# radio_tool

TYT/BaoFeng/(Others?) Firmware tool

# Building
Dependencies:
```
sudo apt install libusb-1.0-0-dev cmake gcc
```

```
git clone https://github.com/v0l/radio_tool
cd radio_tool
mkdir build && cd build
cmake ..
make -j4
./radio_tool --help
```
