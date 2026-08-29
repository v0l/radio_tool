//! Read, write and inspect amateur radio firmware and codeplug files.
//!
//! This is the Rust port of radio_tool. It currently covers the commands that
//! work on files. The ones that need a radio attached, flashing and reading
//! codeplugs over USB, serial or Bluetooth, are still only in the C++ tool.

mod format;

use clap::{Parser, Subcommand};
use firmware::{keyguess, sgl, tyt};
use format::Firmware;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "radio-tool", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print what a firmware file is
    FwInfo {
        /// The firmware file
        input: PathBuf,
        /// Read it as this radio's container. Needed only for the Ailunce and
        /// Yaesu formats, which have no header to identify them by.
        #[arg(short, long)]
        radio: Option<String>,
    },

    /// Print what a codeplug file holds
    CodeplugInfo {
        /// The codeplug file
        input: PathBuf,
    },

    /// Build a firmware file from one or more raw segments
    Wrap {
        /// Radio model, as listed by `models`
        #[arg(short, long)]
        radio: String,
        /// A segment, as address:file, for example 0x0800c000:firmware.bin
        #[arg(short, long = "segment", required = true)]
        segments: Vec<String>,
        /// Where to write the firmware file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Take a firmware file apart into its raw segments
    Unwrap {
        /// The firmware file
        input: PathBuf,
        /// Prefix for the segment files, the address is appended
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Guess the XOR key of a firmware for a radio nobody has looked at yet
    MakeXor {
        /// The firmware file
        input: PathBuf,
    },

    /// Write a codeplug to a radio, over a cable or over Bluetooth
    WriteCodeplug {
        /// The codeplug file
        #[arg(short, long)]
        input: PathBuf,
        /// The serial port, for example /dev/ttyUSB0
        #[arg(short, long, conflicts_with = "ble")]
        port: Option<String>,
        /// Bluetooth address of the radio
        #[arg(long)]
        ble: Option<String>,
        /// Bluetooth adapter to use
        #[arg(long)]
        ble_adapter: Option<String>,
        /// Which radio, see the models command
        #[arg(short, long)]
        radio: String,
        /// Actually write. Without this it only checks the file.
        #[arg(long)]
        write: bool,
    },

    /// Read a codeplug out of a TYT radio in DFU mode, over USB.
    ///
    /// Put the radio in DFU mode first: hold PTT and the button above it
    /// while switching on. The screen stays dark.
    UsbReadCodeplug {
        /// Where to write it
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Read a codeplug out of a radio, over a cable or over Bluetooth
    ReadCodeplug {
        /// The serial port, for example /dev/ttyUSB0
        #[arg(short, long, conflicts_with = "ble", required_unless_present = "ble")]
        port: Option<String>,
        /// Bluetooth address of the radio, for the models that support it
        #[arg(long)]
        ble: Option<String>,
        /// Bluetooth adapter to use, when this machine has more than one
        #[arg(long)]
        ble_adapter: Option<String>,
        /// Radio model, as listed by `models`
        #[arg(short, long)]
        radio: String,
        /// Where to write the codeplug image
        #[arg(short, long)]
        output: PathBuf,
    },

    /// List serial ports this machine can see
    Ports,

    /// Read the flash out of a radio running OpenRTX, over its serial port.
    ///
    /// On the radio: Backup & Restore, then Flash Backup, then press PTT.
    RtxBackup {
        /// The serial port the radio appears as, usually /dev/ttyACM0
        #[arg(short, long, default_value = "/dev/ttyACM0")]
        port: String,
        /// Where to write it
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Write a codeplug image to a TYT radio in DFU mode.
    UsbWriteCodeplug {
        /// The codeplug image
        #[arg(short, long)]
        input: PathBuf,
        /// Actually write. Without this it only checks the file.
        #[arg(long)]
        write: bool,
    },

    /// Read the clock out of a TYT radio on USB. This only reads.
    UsbTime,

    /// Set the clock on a TYT radio on USB.
    ///
    /// The radio keeps local time, so this sends the host's local time
    /// unless one is given.
    UsbSetTime {
        /// A time to set instead of now, as YYYY-MM-DD HH:MM:SS
        #[arg(long)]
        time: Option<String>,
    },

    /// Restart a radio that is in DFU mode.
    UsbReboot,

    /// Read a radio's bootloader out to a file. This only reads.
    UsbDumpBootloader {
        /// Where to write it
        #[arg(short, long)]
        output: PathBuf,
    },

    /// List radios plugged in over USB
    UsbList,

    /// Ask a radio on USB what it is. This only reads.
    UsbInfo,

    /// Dump the vendor registers of a TYT radio on USB. This only reads.
    UsbRegisters,

    /// Write firmware to a radio on USB. This is the one that can brick it.
    Flash {
        /// The firmware file, wrapped in its container
        #[arg(short, long)]
        input: PathBuf,
        /// Actually write. Without this it only says what it would do.
        #[arg(long)]
        write: bool,
    },

    /// Tell a radio in DFU mode to start the firmware it has. Writes nothing.
    UsbLeave {
        /// Where the firmware starts, if not the start of writable flash
        #[arg(long)]
        address: Option<String>,
    },

    /// Read the radio's firmware back out to a file, as a backup. Only reads.
    UsbDump {
        /// Where to write it
        #[arg(short, long)]
        output: PathBuf,
        /// Which memory, as reported by usb-info
        #[arg(long, default_value_t = 0)]
        alt: u8,
    },

    /// Scan for radios advertising over Bluetooth
    BleScan {
        /// How long to scan for
        #[arg(long, default_value_t = 10)]
        seconds: u64,
        /// Bluetooth adapter to use
        #[arg(long)]
        adapter: Option<String>,
    },

    /// List the radios this tool knows about
    Models,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("Error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::FwInfo { input, radio } => fw_info(&input, radio.as_deref()),
        Command::CodeplugInfo { input } => codeplug_info(&input),
        Command::Wrap {
            radio,
            segments,
            output,
        } => wrap(&radio, &segments, &output),
        Command::Unwrap { input, output } => unwrap(&input, &output),
        Command::MakeXor { input } => make_xor(&input),
        Command::ReadCodeplug {
            port,
            ble,
            ble_adapter,
            radio,
            output,
        } => read_codeplug(
            port.as_deref(),
            ble.as_deref(),
            ble_adapter.as_deref(),
            &radio,
            &output,
        ),
        Command::WriteCodeplug {
            input,
            port,
            ble,
            ble_adapter,
            radio,
            write: do_write,
        } => write_codeplug(
            &input,
            port.as_deref(),
            ble.as_deref(),
            ble_adapter.as_deref(),
            &radio,
            do_write,
        ),
        Command::RtxBackup { port, output } => rtx_backup(&port, &output),
        Command::UsbReadCodeplug { output } => usb_read_codeplug(&output),
        Command::UsbWriteCodeplug { input, write } => usb_write_codeplug(&input, write),
        Command::UsbTime => usb_time(),
        Command::UsbSetTime { time } => usb_set_time(time.as_deref()),
        Command::UsbReboot => usb_reboot(),
        Command::UsbDumpBootloader { output } => usb_dump_bootloader(&output),
        Command::UsbList => usb_list(),
        Command::UsbInfo => usb_info(),
        Command::UsbRegisters => usb_registers(),
        Command::UsbDump { output, alt } => usb_dump(&output, alt),
        Command::Flash { input, write } => flash(&input, write),
        Command::UsbLeave { address } => usb_leave(address.as_deref()),
        Command::BleScan { seconds, adapter } => ble_scan(seconds, adapter.as_deref()),
        Command::Ports => {
            for port in device::serial::SerialStream::list() {
                println!("{port}");
            }
            Ok(())
        }
        Command::Models => {
            models();
            Ok(())
        }
    }
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn write(path: &Path, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn fw_info(input: &Path, radio: Option<&str>) -> Result<(), String> {
    let data = read(input)?;
    let fw = match radio {
        Some(radio) => Firmware::parse_as(&data, radio)?,
        None => Firmware::identify(&data).ok_or_else(|| {
            "firmware file not supported. If it is an Ailunce or Yaesu file, \
             say which radio with --radio: those formats have no header"
                .to_owned()
        })?,
    };
    print!("{}", fw.describe());
    Ok(())
}

fn codeplug_info(input: &Path) -> Result<(), String> {
    let data = read(input)?;

    let codeplug = codeplug::parse(&data).map_err(|_| "codeplug file not supported".to_owned())?;
    println!("{codeplug}");
    Ok(())
}

/// A segment argument is address:path, with the address in hex or decimal
fn parse_segment(arg: &str) -> Result<(u32, PathBuf), String> {
    let (addr, path) = arg
        .split_once(':')
        .ok_or_else(|| format!("segments must be address:file, got {arg:?}"))?;

    let addr = addr.trim();
    let parsed = match addr.strip_prefix("0x").or_else(|| addr.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => addr.parse(),
    };
    let parsed = parsed.map_err(|e| format!("bad segment address {addr:?}: {e}"))?;

    Ok((parsed, PathBuf::from(path)))
}

fn wrap(radio: &str, segments: &[String], output: &Path) -> Result<(), String> {
    let parsed: Vec<(u32, PathBuf)> = segments
        .iter()
        .map(|s| parse_segment(s))
        .collect::<Result<_, _>>()?;

    // the TYT container takes several regions, the SGL container one
    if tyt::config_for_model(radio).is_some() {
        let mut fw = tyt::TytFirmware::new(radio).map_err(|e| e.to_string())?;
        for (address, path) in &parsed {
            let data = read(path)?;
            eprintln!("Adding segment {address:#010x} from {}", path.display());
            fw.append_segment(*address, &data, 0)
                .map_err(|e| e.to_string())?;
        }
        return write(output, &fw.serialise().map_err(|e| e.to_string())?);
    }

    if sgl::config_for_model(radio).is_some() {
        let [(address, path)] = parsed.as_slice() else {
            return Err("this radio takes exactly one segment".to_owned());
        };
        if *address != 0 {
            return Err(format!(
                "this radio writes from address 0, not {address:#x}"
            ));
        }
        let data = read(path)?;
        eprintln!("Adding segment {address:#010x} from {}", path.display());
        let mut fw = sgl::SglFirmware::new(radio).map_err(|e| e.to_string())?;
        fw.set_data(&data).map_err(|e| e.to_string())?;
        return write(output, &fw.serialise().map_err(|e| e.to_string())?);
    }

    Err(format!(
        "unknown radio {radio:?}, see the models command for the list"
    ))
}

fn unwrap(input: &Path, output: &Path) -> Result<(), String> {
    let data = read(input)?;
    let fw = Firmware::identify(&data).ok_or_else(|| "firmware file not supported".to_owned())?;

    for segment in fw.segments() {
        let name = format!("{}_{:#010x}", output.display(), segment.address);
        eprintln!("Writing {} bytes to {name}", segment.data.len());
        write(Path::new(&name), segment.data)?;
    }
    Ok(())
}

fn make_xor(input: &Path) -> Result<(), String> {
    let data = read(input)?;
    let encrypted = tyt::encrypted_payload(&data).map_err(|e| e.to_string())?;

    if encrypted.len() < 64 * keyguess::KEY_LEN {
        eprintln!(
            "Warning: {} bytes is not much to guess from, expect a poor key",
            encrypted.len()
        );
    }

    let key = keyguess::guess_key(encrypted);

    // if the file is one we know, say how well the guess did
    if let Ok(fw) = tyt::TytFirmware::parse(&data) {
        let agreement = keyguess::key_agreement(&key, fw.config().cipher);
        eprintln!(
            "This is a known {} firmware, the guess matches its key {:.1}% of the time",
            fw.config().radio_model,
            agreement * 100.0
        );
    }

    for (ix, chunk) in key.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        println!("{:04x}: {}", ix * 16, hex.join(" "));
    }
    Ok(())
}

/// The UV-5R family clones at this rate, the UV-17Pro family faster
const UV5R_BAUD: u32 = 9600;
const UV17PRO_BAUD: u32 = 115_200;

fn usb_list() -> Result<(), String> {
    let found = device::usb::list().map_err(|e| e.to_string())?;

    if found.is_empty() {
        eprintln!(
            "No radios found on USB. A radio only appears here in bootloader \
             or DFU mode, not in normal use."
        );
        return Ok(());
    }

    for radio in &found {
        println!("{radio}");
    }
    Ok(())
}

/// Ask each radio on USB what it is.
///
/// Reading only. Nothing here writes to a radio, which is why it is safe to
/// run against a device whose model is not yet known.
fn usb_info() -> Result<(), String> {
    use device::usb::{Protocol, UsbDevice};

    let found = device::usb::list().map_err(|e| e.to_string())?;
    if found.is_empty() {
        eprintln!("No radios found on USB. Is the radio in bootloader or DFU mode?");
        return Ok(());
    }

    let timeout = std::time::Duration::from_millis(5000);

    for radio in &found {
        println!("{radio}");

        let mut usb = UsbDevice::open(radio.known.vendor, radio.known.product, timeout)
            .map_err(|e| e.to_string())?;

        match radio.known.protocol {
            Protocol::TytDfu => {
                // ask the radio to describe its own memory before anything
                // else, since this is what says where writing is permitted
                match usb.memory_layout() {
                    Ok(layout) => {
                        for memory in &layout {
                            println!("  {memory}");
                            let programmable: u64 = memory
                                .programmable()
                                .iter()
                                .map(|s| u64::from(s.size))
                                .sum();
                            if programmable != memory.size() {
                                println!("    of which {programmable} bytes may be programmed");
                            }
                        }
                    }
                    Err(e) => println!("  the radio did not describe its memory: {e}"),
                }

                let mut dfu = device::dfu::Dfu::new(usb);
                match device::dfu::tyt::identify(&mut dfu) {
                    Ok(model) => println!("  model: {model}"),
                    Err(e) => println!("  could not read the model: {e}"),
                }
                match dfu.state() {
                    Ok(state) => println!("  DFU state: {state:?}"),
                    Err(e) => println!("  could not read the state: {e}"),
                }
            }
            Protocol::H8sx => {
                usb.claim(0).map_err(|e| e.to_string())?;
                match device::h8sx::identify(&mut usb) {
                    Ok(id) => println!("  device: {id}"),
                    Err(e) => println!("  could not identify it: {e}"),
                }
            }
            Protocol::TytHid => {
                println!("  the HID protocol for this radio is not implemented yet");
            }
        }
    }

    Ok(())
}

/// Dump every vendor register, reading only.
fn usb_registers() -> Result<(), String> {
    use device::dfu::{Dfu, tyt};
    use device::usb::{Protocol, UsbDevice};

    let found = device::usb::list().map_err(|e| e.to_string())?;
    let radio = found
        .iter()
        .find(|r| r.known.protocol == Protocol::TytDfu)
        .ok_or("no TYT radio in DFU mode on USB")?;

    let usb = UsbDevice::open(
        radio.known.vendor,
        radio.known.product,
        std::time::Duration::from_millis(5000),
    )
    .map_err(|e| e.to_string())?;
    let mut dfu = Dfu::new(usb);

    println!("{radio}");
    if let Ok(status) = dfu.status() {
        println!(
            "  status {:#04x}, state {:?}, poll timeout {} ms",
            status.status, status.state, status.poll_timeout
        );
    }

    for (name, register) in [
        ("RadioInfo", tyt::Register::RadioInfo),
        ("R02", tyt::Register::R02),
        ("R03", tyt::Register::R03),
        ("R04", tyt::Register::R04),
        ("R07", tyt::Register::R07),
        ("RTC", tyt::Register::Rtc),
    ] {
        match tyt::read_register(&mut dfu, register) {
            Ok(data) => {
                // the register is a fixed size buffer, so show the part that
                // is not padding rather than a kilobyte of 0xff
                let end = data
                    .iter()
                    .rposition(|b| *b != 0xff)
                    .map_or(0, |i| i + 1)
                    .min(64);
                println!(
                    "  {name:<9} ({:#04x}): {}",
                    register as u8,
                    hex(data.get(..end).unwrap_or(&data))
                );
            }
            Err(e) => println!("  {name:<9} ({:#04x}): {e}", register as u8),
        }
    }

    Ok(())
}

fn hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Open the first TYT radio in DFU mode
fn open_tyt_dfu(timeout: std::time::Duration) -> Result<device::usb::UsbDevice, String> {
    use device::usb::{Protocol, UsbDevice};

    let found = device::usb::list().map_err(|e| e.to_string())?;
    let radio = found
        .iter()
        .find(|r| r.known.protocol == Protocol::TytDfu)
        .ok_or("no TYT radio in DFU mode on USB")?;

    UsbDevice::open(radio.known.vendor, radio.known.product, timeout).map_err(|e| e.to_string())
}

/// Read a radio's clock.
///
/// The raw register is shown alongside, because on some radios it is not a
/// clock: a DM-1701 answers with `4b 01 00 10`, which the C++ reads as the
/// year 5100 rather than as evidence there is no clock there.
fn usb_time() -> Result<(), String> {
    use device::dfu::{Dfu, tyt};

    let usb = open_tyt_dfu(std::time::Duration::from_secs(10))?;
    let mut dfu = Dfu::new(usb);

    let (stamp, raw) = tyt::read_clock(&mut dfu).map_err(|e| e.to_string())?;
    println!("register {:#04x}: {}", tyt::Register::Rtc as u8, hex(&raw));

    match stamp {
        Some(t) => println!("clock: {t}"),
        None => println!("clock: this register does not hold a timestamp on this radio"),
    }
    Ok(())
}

/// Set a radio's clock.
///
/// The radio keeps local time. It has no notion of a timezone, so sending it
/// UTC would leave it wrong by the offset.
fn usb_set_time(given: Option<&str>) -> Result<(), String> {
    use chrono::{Datelike, Local, NaiveDateTime, Timelike};
    use device::dfu::{Dfu, tyt};

    let when = match given {
        Some(text) => NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S")
            .map_err(|e| format!("cannot read {text:?} as a time: {e}"))?,
        None => Local::now().naive_local(),
    };

    let stamp = tyt::Timestamp {
        year: u16::try_from(when.year()).map_err(|_| "that year is out of range")?,
        month: when.month() as u8,
        day: when.day() as u8,
        hour: when.hour() as u8,
        minute: when.minute() as u8,
        second: when.second() as u8,
    };

    let usb = open_tyt_dfu(std::time::Duration::from_secs(10))?;
    let mut dfu = Dfu::new(usb);

    println!("Setting the clock to {stamp}");
    tyt::write_clock(&mut dfu, stamp).map_err(|e| e.to_string())?;

    // read it straight back, so a clock that did not take is obvious
    match tyt::read_clock(&mut dfu) {
        Ok((Some(now), _)) => println!("The radio now says {now}"),
        Ok((None, raw)) => println!("The radio answered {}, which is not a time", hex(&raw)),
        Err(e) => println!("Set, but reading it back failed: {e}"),
    }
    Ok(())
}

/// Restart a radio in DFU mode.
fn usb_reboot() -> Result<(), String> {
    use device::dfu::{Dfu, tyt};

    // where the radio is now. A radio running its own firmware comes back as
    // a DFU device after a restart, because that is what it looks like when
    // it is switched on, so its presence afterwards says nothing. Its bus
    // address changing is what says it restarted.
    let before = device::usb::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|r| r.known.protocol == device::usb::Protocol::TytDfu)
        .map(|r| (r.bus.clone(), r.address));

    let usb = open_tyt_dfu(std::time::Duration::from_secs(5))?;
    let mut dfu = Dfu::new(usb);

    // the radio goes away without answering the second command, which is not
    // a failure
    tyt::reboot(&mut dfu).map_err(|e| e.to_string())?;
    drop(dfu);

    // a radio that restarted leaves the bus. One that is still here ignored
    // the command, which a bootloader does: these are vendor commands the
    // stock firmware implements, not the bootloader.
    std::thread::sleep(std::time::Duration::from_secs(3));

    let after = device::usb::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|r| r.known.protocol == device::usb::Protocol::TytDfu)
        .map(|r| (r.bus.clone(), r.address));

    match (before, after) {
        (Some(was), None) => {
            let _ = was;
            println!("Rebooted: the radio has left the bus.");
        }
        (Some(was), Some(now)) if was != now => {
            println!("Rebooted: the radio came back at a new address.");
        }
        (Some(_), Some(_)) => {
            println!(
                "Sent, and the radio is at the same address, so it ignored it. \
                 A bootloader does: these are vendor commands the radio's own \
                 firmware implements. Switch it off and on instead."
            );
        }
        (None, _) => println!("Sent."),
    }
    Ok(())
}

/// Read the bootloader out of a TYT radio.
///
/// One transfer of 0xc000 bytes. The C++ does the same and is marked as
/// working on macOS only; it works here because nusb does not impose the
/// limit that made it fail.
fn usb_dump_bootloader(output: &Path) -> Result<(), String> {
    use device::dfu::ControlTransfer;

    /// The bootloader occupies the first 48 kB of flash
    const BOOTLOADER_LEN: usize = 0xc000;

    let mut usb = open_tyt_dfu(std::time::Duration::from_secs(30))?;
    usb.select_alt(0, 0).map_err(|e| e.to_string())?;

    // 0x02 is the DFU upload request, and block numbers start at two. No
    // address is set: this reads from the start of flash, and setting one
    // would leave the device in a state where an upload is not allowed.
    let data = usb
        .control_in(0x02, 2, BOOTLOADER_LEN)
        .map_err(|e| e.to_string())?;

    if data.first() == Some(&0x21) {
        return Err(
            "the radio echoed a command back instead of reading memory. Unplug it, \
             put it back in DFU mode, and read before writing anything"
                .to_owned(),
        );
    }

    let used = data.iter().rposition(|b| *b != 0xff).map_or(0, |i| i + 1);
    println!(
        "Read {} of bootloader, {} of it used",
        format::format_bytes(data.len() as u64),
        format::format_bytes(used as u64)
    );

    write(output, &data)?;
    Ok(())
}

/// Write a codeplug image to a radio over DFU.
///
/// This erases the radio's configuration memory first, so whatever was on it
/// is gone. Read it out beforehand if it matters.
fn usb_write_codeplug(input: &Path, do_write: bool) -> Result<(), String> {
    use device::dfu::{Dfu, codeplug};
    use device::usb::{Protocol, UsbDevice};

    let image = read(input)?;
    if image.len() != codeplug::MEMORY_SIZE {
        return Err(format!(
            "{} is {} bytes, and a codeplug image is {}",
            input.display(),
            image.len(),
            codeplug::MEMORY_SIZE
        ));
    }

    let found = device::usb::list().map_err(|e| e.to_string())?;
    let radio = found
        .iter()
        .find(|r| r.known.protocol == Protocol::TytDfu)
        .ok_or("no TYT radio in DFU mode on USB")?;

    println!("Codeplug: {} ({} bytes)", input.display(), image.len());
    println!("Radio:    {radio}");

    if !do_write {
        println!();
        println!("Nothing was written. Pass --write to do it.");
        return Ok(());
    }

    let mut usb = UsbDevice::open(
        radio.known.vendor,
        radio.known.product,
        std::time::Duration::from_secs(30),
    )
    .map_err(|e| e.to_string())?;
    usb.claim(0).map_err(|e| e.to_string())?;

    println!();
    println!("Erasing and writing. Do not unplug the radio.");

    let mut dfu = Dfu::new(usb);
    codeplug::write(&mut dfu, &image, |p| {
        eprint!(
            "\r  {} of {}    ",
            format::format_bytes(p.done as u64),
            format::format_bytes(p.total as u64)
        );
    })
    .map_err(|e| e.to_string())?;

    eprintln!();
    println!("Done. Restart the radio.");
    Ok(())
}

/// Read a codeplug out of a TYT radio over DFU.
///
/// Reading only. The sequence is dmrconfig's, which is what people use on
/// these radios: set the address once, then walk the blocks.
fn usb_read_codeplug(output: &Path) -> Result<(), String> {
    use device::dfu::{Dfu, codeplug};
    use device::usb::{Protocol, UsbDevice};

    let found = device::usb::list().map_err(|e| e.to_string())?;
    let radio = found
        .iter()
        .find(|r| r.known.protocol == Protocol::TytDfu)
        .ok_or(
            "no TYT radio in DFU mode on USB. Hold PTT and the button above it while switching on",
        )?;

    let mut usb = UsbDevice::open(
        radio.known.vendor,
        radio.known.product,
        std::time::Duration::from_secs(30),
    )
    .map_err(|e| e.to_string())?;

    // dmrconfig claims the interface before it starts, and the radio wants
    // that before it will hand over its configuration memory
    usb.claim(0).map_err(|e| e.to_string())?;

    println!("{radio}");
    let mut dfu = Dfu::new(usb);

    let model = device::dfu::tyt::identify(&mut dfu).unwrap_or_default();

    let image = codeplug::read(&mut dfu, |p| {
        eprint!(
            "\r  {} of {}    ",
            format::format_bytes(p.done as u64),
            format::format_bytes(p.total as u64)
        );
    })
    .map_err(|e| e.to_string())?;

    eprintln!();
    write(output, &image)?;

    // read it back straight away, so a dump that came off the radio empty is
    // obvious now rather than later
    match ::codeplug::rdt::RdtCodeplug::parse_image(&image, &model) {
        Ok(cp) => println!("{cp}"),
        Err(e) => println!("Saved, but this does not parse as a codeplug: {e}"),
    }
    Ok(())
}

/// Read a radio's flash over OpenRTX's serial port.
///
/// This needs no bootloader: the radio is simply switched on. It does need a
/// person to start the transfer from the radio's own menu, so the wait for
/// the first block is a long one.
fn rtx_backup(port: &str, output: &Path) -> Result<(), String> {
    use device::{serial::SerialStream, xmodem};

    // OpenRTX runs a USB serial port, so the speed is whatever the host
    // likes, and the radio waits for a person before it says anything
    let mut stream = SerialStream::open(port, 115_200, std::time::Duration::from_millis(200))
        .map_err(|e| e.to_string())?;

    eprintln!("Waiting for the radio.");
    eprintln!("On the radio: Backup & Restore, then Flash Backup, then press PTT.");

    let config = xmodem::Config::default();
    let data = xmodem::receive(&mut stream, &config, |p| {
        eprint!(
            "\r  {} in {} blocks    ",
            format::format_bytes(p.received as u64),
            p.blocks
        );
    })
    .map_err(|e| e.to_string())?;

    eprintln!();
    write(output, &data)?;
    Ok(())
}

/// Write a codeplug to a radio.
///
/// A codeplug is not firmware, so a bad write costs the channel memory rather
/// than the radio, but nothing verifies afterwards: the radio acknowledges
/// each block and says nothing else.
fn write_codeplug(
    input: &Path,
    port: Option<&str>,
    ble: Option<&str>,
    ble_adapter: Option<&str>,
    radio: &str,
    do_write: bool,
) -> Result<(), String> {
    use device::{BoxedStream, ble::BleStream, serial::SerialStream, uv17pro};

    let data = read(input)?;

    let model = uv17pro::model(radio)
        .ok_or_else(|| format!("{radio:?} cannot be written to yet, see the models command"))?;

    let total = model.memory_size();
    if data.len() < total {
        return Err(format!(
            "{} is {} bytes, too small for a {radio} which needs {total}",
            input.display(),
            data.len()
        ));
    }

    // a file straight from this tool or from CHIRP carries a stamp after the
    // memory. Anything else is worth mentioning before it is written.
    let tail = data.get(total..).unwrap_or_default();
    let stamped = tail.starts_with(model.chirp_model.as_bytes());
    let from_chirp = tail
        .windows(codeplug::CHIRP_MAGIC.len())
        .any(|w| w == codeplug::CHIRP_MAGIC);

    println!("Codeplug: {} ({} bytes)", input.display(), data.len());
    println!("Radio:    {radio}, memory {total} bytes");
    if !tail.is_empty() && !stamped && !from_chirp {
        println!(
            "  warning: this file is not stamped for a {}, the first {total} bytes \
             would be written anyway",
            model.chirp_model
        );
    }

    if !do_write {
        println!();
        println!("Nothing was written. Pass --write to do it.");
        return Ok(());
    }

    let timeout = std::time::Duration::from_millis(2000);
    let stream: BoxedStream = match (ble, port) {
        (Some(address), _) => {
            eprintln!("Connecting to a {radio} at {address}");
            Box::new(
                BleStream::connect(address, ble_adapter, 15, timeout).map_err(|e| e.to_string())?,
            )
        }
        (None, Some(port)) => {
            eprintln!("Writing the codeplug to a {radio} on {port}");
            Box::new(SerialStream::open(port, UV17PRO_BAUD, timeout).map_err(|e| e.to_string())?)
        }
        (None, None) => return Err("a serial port or a Bluetooth address is needed".to_owned()),
    };

    uv17pro::Session::new(stream, model)
        .upload(&data, |done, all| {
            eprint!("\r  {done} of {all} bytes    ");
        })
        .map_err(|e| e.to_string())?;

    eprintln!();
    println!("Done.");
    Ok(())
}

/// Ask a radio to leave DFU and run what it has.
///
/// This sends no data. It is the end of transfer that a download needs, and
/// is worth trying on its own when a radio has been written to and stayed in
/// DFU afterwards.
fn usb_leave(address: Option<&str>) -> Result<(), String> {
    use device::dfu::Dfu;
    use device::usb::{Protocol, UsbDevice};

    let found = device::usb::list().map_err(|e| e.to_string())?;
    let radio = found
        .iter()
        .find(|r| r.known.protocol == Protocol::TytDfu)
        .ok_or("no TYT radio in DFU mode on USB")?;

    let usb = UsbDevice::open(
        radio.known.vendor,
        radio.known.product,
        std::time::Duration::from_secs(10),
    )
    .map_err(|e| e.to_string())?;

    let layout = usb.memory_layout().map_err(|e| e.to_string())?;
    let start = match address {
        Some(text) => u32::from_str_radix(text.trim_start_matches("0x"), 16)
            .map_err(|_| "the address should be hexadecimal, such as 0x0800c000")?,
        None => layout
            .iter()
            .find(|m| m.alt == 0)
            .and_then(|m| m.programmable().first().map(|s| s.start))
            .ok_or("the radio did not say where its writable flash starts")?,
    };

    println!("Asking the radio to start what is at {start:#010x}");
    let mut dfu = Dfu::new(usb);
    dfu.leave(start).map_err(|e| e.to_string())?;
    println!("Sent. The radio should restart on its own.");
    Ok(())
}

/// Write firmware to a radio.
///
/// Without `--write` this changes nothing and prints what it would do, which
/// is the sensible thing to look at first.
fn flash(input: &Path, do_write: bool) -> Result<(), String> {
    use device::dfu::{Dfu, flashing};
    use device::usb::{Protocol, UsbDevice};

    let data = read(input)?;
    let firmware = format::Firmware::identify(&data)
        .ok_or("this file is not a firmware container this tool recognises")?;

    let found = device::usb::list().map_err(|e| e.to_string())?;
    let radio = found
        .first()
        .ok_or("no radio found on USB. Is it in bootloader or DFU mode?")?;

    // an SGL radio speaks the HID protocol and knows its own memory, so it
    // needs none of the sector work below
    if radio.known.protocol == Protocol::TytHid {
        return flash_hid(&firmware, radio, do_write);
    }

    let usb = UsbDevice::open(
        radio.known.vendor,
        radio.known.product,
        std::time::Duration::from_secs(30),
    )
    .map_err(|e| e.to_string())?;

    // what the radio says may be written, which is not the same as what the
    // chip has
    let layout = usb.memory_layout().map_err(|e| e.to_string())?;
    let internal = layout
        .iter()
        .find(|m| m.alt == 0)
        .ok_or("the radio did not describe its internal flash")?;
    let map = internal.programmable();

    // the radio is sent the stored form, not the deciphered image
    let stored = firmware.segments_as_stored()?;
    let all: Vec<flashing::Region<'_>> = stored
        .iter()
        .map(|(address, data)| flashing::Region {
            address: *address,
            data,
        })
        .collect();

    println!("Firmware: {} for {}", input.display(), firmware.radio());
    println!("Radio:    {radio}");
    println!("Memory:   {internal}");
    println!();

    // A vendor firmware file can carry regions for memory this operation
    // cannot reach: a stock DM-1701 image holds its resources for the SPI
    // flash alongside the firmware for the internal flash. The C++ drops
    // those silently because its hardcoded map does not contain them. Say so
    // instead, and write what can be written.
    let (regions, skipped): (Vec<&flashing::Region<'_>>, Vec<&flashing::Region<'_>>) =
        all.iter().partition(|region| {
            let end = region.address + region.data.len() as u32;
            let covered: u32 = device::flash::split(&map, region.address, end)
                .iter()
                .map(|p| p.length)
                .sum();
            covered == end.saturating_sub(region.address)
        });

    for region in &skipped {
        println!(
            "  skipping {:#010x} to {:#010x} ({}): outside the memory this radio \
             offers for writing",
            region.address,
            region.address + region.data.len() as u32,
            format::format_bytes(region.data.len() as u64)
        );
    }
    if !skipped.is_empty() {
        println!();
    }

    if regions.is_empty() {
        return Err("none of this firmware falls in memory the radio will write".to_owned());
    }

    let regions: Vec<flashing::Region<'_>> = regions.into_iter().copied().collect();

    for region in &regions {
        let end = region.address + region.data.len() as u32;
        println!(
            "  {:#010x} to {:#010x}  {}",
            region.address,
            end,
            format::format_bytes(region.data.len() as u64)
        );
        for piece in device::flash::split(&map, region.address, end) {
            println!(
                "    erase and write {} of {}",
                format::format_bytes(u64::from(piece.length)),
                piece.sector
            );
        }
    }

    if !do_write {
        println!();
        println!("Nothing was written. Pass --write to do it.");
        return Ok(());
    }

    let model = firmware.radio();
    println!();
    println!("Writing {model} firmware. Do not unplug the radio.");

    let mut dfu = Dfu::new(usb);
    flashing::write(&mut dfu, &regions, &map, |step| match step {
        flashing::Step::Erasing { address, .. } => {
            eprint!("\r  erasing {address:#010x}          ");
        }
        flashing::Step::Writing { written, total, .. } => {
            eprint!(
                "\r  writing {} of {}      ",
                format::format_bytes(written as u64),
                format::format_bytes(total as u64)
            );
        }
    })
    .map_err(|e| e.to_string())?;

    eprintln!();
    println!("Done. Restart the radio.");
    Ok(())
}

/// Write firmware to a radio that speaks the HID protocol, which is the
/// Radioddity GD-77 and its relatives.
///
/// The radio erases its own flash, so there is no sector arithmetic here and
/// nothing to decide about what may be written.
fn flash_hid(
    firmware: &format::Firmware,
    radio: &device::usb::Found,
    do_write: bool,
) -> Result<(), String> {
    use device::hid::{Session, flashing};
    use device::usb::UsbDevice;

    let identity = firmware
        .sgl_identity()
        .ok_or("this firmware has no model key, so it cannot open a session")?;
    let stored = firmware.segments_as_stored()?;
    let data = stored
        .first()
        .map(|(_, d)| d.clone())
        .ok_or("this firmware has no data")?;

    println!("Firmware: for {}", firmware.radio());
    println!("Radio:    {radio}");
    println!(
        "  model key {}, group {}, version {}",
        String::from_utf8_lossy(&identity.model_key),
        String::from_utf8_lossy(&identity.radio_group),
        String::from_utf8_lossy(&identity.protocol_version)
    );
    println!("  {} to write", format::format_bytes(data.len() as u64));

    if !do_write {
        println!();
        println!("Nothing was written. Pass --write to do it.");
        return Ok(());
    }

    let mut usb = UsbDevice::open(
        radio.known.vendor,
        radio.known.product,
        std::time::Duration::from_secs(30),
    )
    .map_err(|e| e.to_string())?;
    usb.claim_hid().map_err(|e| e.to_string())?;

    println!();
    println!("Writing. Do not unplug the radio.");

    let mut session = Session::new(usb);
    flashing::write(&mut session, &identity, &data, |p| {
        eprint!(
            "\r  {} of {}      ",
            format::format_bytes(p.written as u64),
            format::format_bytes(p.total as u64)
        );
    })
    .map_err(|e| e.to_string())?;

    eprintln!();
    println!("Done. Restart the radio.");
    Ok(())
}

/// Read what can be read of a radio's memory.
///
/// Reading only, and on a TYT radio it reads less than you would hope. Its
/// DFU upload ignores both the address pointer and the block number and
/// always returns a linear read from the start of flash, so the most that can
/// be had is one transfer, and USB caps a control transfer at 65535 bytes.
///
/// That is enough for the bootloader and the start of the application, and
/// not enough for a backup you could restore. Rather than hand back a file
/// that looks like a backup and is not, this refuses to write one if the
/// radio is repeating itself.
fn usb_dump(output: &Path, alt: u8) -> Result<(), String> {
    use device::dfu::ControlTransfer;
    use device::usb::{Protocol, UsbDevice};

    /// The most a USB control transfer can carry
    const MAX_TRANSFER: usize = 0xffff;

    let found = device::usb::list().map_err(|e| e.to_string())?;
    let radio = found
        .iter()
        .find(|r| r.known.protocol == Protocol::TytDfu)
        .ok_or("no TYT radio in DFU mode on USB")?;

    let mut usb = UsbDevice::open(
        radio.known.vendor,
        radio.known.product,
        std::time::Duration::from_secs(30),
    )
    .map_err(|e| e.to_string())?;

    let layout = usb.memory_layout().map_err(|e| e.to_string())?;
    let memory = layout
        .iter()
        .find(|m| m.alt == alt)
        .ok_or_else(|| format!("the radio has no memory at alternate setting {alt}"))?;

    usb.select_alt(0, alt).map_err(|e| e.to_string())?;

    // a radio left in a download state stalls an upload, so bring it back to
    // idle first, which is what dfu-util does before reading
    {
        let mut dfu = device::dfu::Dfu::new(&mut usb);
        dfu.abort().map_err(|e| e.to_string())?;
        let _ = dfu.status();
    }

    // 0x02 is the DFU upload request, and block numbers start at two
    let image = usb
        .control_in(0x02, 2, MAX_TRANSFER)
        .map_err(|e| e.to_string())?;

    if image.is_empty() {
        return Err("the radio returned nothing".to_owned());
    }

    // a radio that ignores addressing hands back the same block over and
    // over, which would be a useless file that looks like a good one
    let distinct = image
        .chunks(1024)
        .collect::<std::collections::HashSet<_>>()
        .len();
    if distinct <= 1 && image.len() > 1024 {
        return Err(
            "the radio returned the same block repeatedly, so this would not be a backup"
                .to_owned(),
        );
    }

    // after any download, a TYT radio answers an upload with the buffer of
    // the last command it was sent rather than with memory. It starts 0x21
    // followed by the address, and it is not a backup either.
    if image.first() == Some(&0x21) {
        return Err(
            "the radio echoed the last command back instead of reading memory. \
             Unplug it, put it back in DFU mode, and read before writing anything"
                .to_owned(),
        );
    }

    eprintln!(
        "Read {} of {}. This radio cannot be read further: its DFU upload ignores \
         addressing, so this is the start of flash and not a restorable backup.",
        format::format_bytes(image.len() as u64),
        memory.name
    );

    write(output, &image)?;
    Ok(())
}

fn ble_scan(seconds: u64, adapter: Option<&str>) -> Result<(), String> {
    eprintln!("Scanning for {seconds} seconds...");
    let found = device::ble::scan(seconds, adapter).map_err(|e| e.to_string())?;

    let named: Vec<_> = found.iter().filter(|d| !d.name.is_empty()).collect();
    if named.is_empty() {
        eprintln!(
            "Nothing advertising a name. {} devices seen in total.",
            found.len()
        );
        return Ok(());
    }
    for device in named {
        println!("{}  {}", device.address, device.name);
    }
    Ok(())
}

fn read_codeplug(
    port: Option<&str>,
    ble: Option<&str>,
    ble_adapter: Option<&str>,
    radio: &str,
    output: &Path,
) -> Result<(), String> {
    use device::{BoxedStream, ble::BleStream, serial::SerialStream, uv5r, uv17pro};

    // a Bluetooth radio answers slowly, and the first reply after connecting
    // has been measured at over a second, so neither link gets a tight one
    let timeout = std::time::Duration::from_millis(2000);

    // which driver, and how fast the cable runs for it
    let (is_uv5r, baud) = match (uv5r::model(radio), uv17pro::model(radio)) {
        (Some(_), _) => (true, UV5R_BAUD),
        (None, Some(_)) => (false, UV17PRO_BAUD),
        (None, None) => {
            return Err(format!(
                "{radio:?} does not clone over a cable or Bluetooth, see the models command"
            ));
        }
    };

    // the transport is chosen separately from the driver, which is the whole
    // point of the stream being a trait object
    let stream: BoxedStream = match (ble, port) {
        (Some(_), _) if is_uv5r => {
            return Err(format!(
                "the {radio} has no Bluetooth, it clones over a cable only. The UV-5R Mini \
                 is the one in this family that does: --radio UV5RMINI"
            ));
        }
        (Some(address), _) => {
            eprintln!("Connecting to a {radio} at {address}");
            Box::new(
                BleStream::connect(address, ble_adapter, 15, timeout).map_err(|e| e.to_string())?,
            )
        }
        (None, Some(port)) => {
            eprintln!("Reading the codeplug from a {radio} on {port}");
            Box::new(SerialStream::open(port, baud, timeout).map_err(|e| e.to_string())?)
        }
        (None, None) => return Err("a serial port or a Bluetooth address is needed".to_owned()),
    };

    let (image, described) = if is_uv5r {
        let model = uv5r::model(radio).ok_or("unknown radio")?;
        let (image, info) = uv5r::Session::new(stream, model)
            .download()
            .map_err(|e| e.to_string())?;
        (image, info.firmware)
    } else {
        let model = uv17pro::model(radio).ok_or("unknown radio")?;
        let (image, info) = uv17pro::Session::new(stream, model)
            .download()
            .map_err(|e| e.to_string())?;
        (image, info.describe())
    };

    if !described.is_empty() {
        eprintln!("Radio reports: {described}");
    }
    write(output, &image)?;
    eprintln!("Wrote {} bytes to {}", image.len(), output.display());
    Ok(())
}

fn models() {
    println!("Firmware:");
    for config in tyt::ALL {
        println!("  {:<12} TYT container", config.radio_model);
    }
    for config in sgl::ALL {
        println!("  {:<12} SGL container", config.radio_model);
    }
    println!(
        "  {:<12} Connect Systems container",
        firmware::cs::RADIO_MODEL
    );
    println!("  {:<12} Ailunce container", firmware::ailunce::RADIO_MODEL);
    for model in firmware::yaesu::RADIO_MODELS {
        println!("  {model:<12} Yaesu container");
    }

    println!("\nRadios that clone over a serial cable:");
    for m in device::uv5r::ALL {
        println!("  {:<12} Baofeng UV-5R family", m.name);
    }
    for m in device::uv17pro::ALL {
        println!("  {:<12} Baofeng {}", m.name, m.chirp_model);
    }

    println!("\nCodeplugs:");
    println!("  {:<12} Baofeng UV-5R family", "UV5R");
    println!("  {:<12} TYT and Baofeng DM-1701", "RDT");
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn segment_arguments_take_hex_or_decimal() {
        assert_eq!(
            parse_segment("0x0800c000:fw.bin").unwrap(),
            (0x0800_c000, PathBuf::from("fw.bin"))
        );
        assert_eq!(
            parse_segment("4096:fw.bin").unwrap(),
            (4096, PathBuf::from("fw.bin"))
        );
        // a path with a colon in it still works, only the first splits
        assert_eq!(
            parse_segment("0x0:a:b.bin").unwrap(),
            (0, PathBuf::from("a:b.bin"))
        );
    }

    #[test]
    fn bad_segment_arguments_are_rejected() {
        // the C++ used stoi here, so any address over 0x7fffffff threw
        assert!(parse_segment("0xffffffff:fw.bin").is_ok());
        assert!(parse_segment("0x100000000:fw.bin").is_err());
        assert!(parse_segment("no-colon").is_err());
        assert!(parse_segment("notanumber:fw.bin").is_err());
        assert!(parse_segment(":fw.bin").is_err());
    }

    #[test]
    fn the_command_line_parses() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
