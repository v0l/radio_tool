# Changelog

## [0.5.0]

- A window. `radio_tool` with no arguments, or `radio_tool-ui`, lists the
  radios on USB, a serial cable or Bluetooth, and reads, writes and flashes
  them. Writing and flashing ask first. Drop a firmware or codeplug file on it
  to see what it is, with the channels in a table.
- Builds for Windows and macOS: a zip, an installer and an app.
- Rust 1.95 or newer to build.

## [0.4.0]

radio_tool is now written in Rust. It takes the same flags as 0.3.0 and
prints the same output, so scripts keep working.

- Subcommands (`radio_tool help`) alongside the old flags. The ones that
  write only check unless given `--write`.
- Read and write DM-1701 and other TYT codeplugs over USB.
- Read and write codeplugs on the Baofeng UV-5R and UV-17Pro families, over a
  cable or Bluetooth.
- `--codeplug-info` reads CHIRP UV-5R images.
- `--wrap` works for the CS800, FT-70D and HD1 as well as TYT and SGL radios.
- No libusb, cmake or C++ toolchain needed. Linux needs the udev rules in the
  archive to reach a radio without root.

Not yet tried on a radio: flashing an Ailunce HD1 or Yaesu FT-70D, writing a
UV-17Pro codeplug, and writing a UV-5R codeplug.

[0.5.0]: https://github.com/v0l/radio_tool/releases/tag/v0.5.0
[0.4.0]: https://github.com/v0l/radio_tool/releases/tag/v0.4.0
