mod args;
mod text;

use args::Parsed;
use device::usb::{Found, Protocol, UsbDevice};
use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

enum Stop {
    Fail(String),
    Code(u8),
}

impl From<String> for Stop {
    fn from(message: String) -> Self {
        Self::Fail(message)
    }
}

impl From<&str> for Stop {
    fn from(message: &str) -> Self {
        Self::Fail(message.to_owned())
    }
}

type Outcome = Result<(), Stop>;

pub fn run(argv: &[OsString]) -> ExitCode {
    let prog = argv
        .first()
        .map(|a| a.to_string_lossy().into_owned())
        .unwrap_or_else(|| "radio_tool".to_owned());
    let args: Vec<String> = argv
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();

    let parsed = match args::parse(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error parsing options: {e}");
            return ExitCode::FAILURE;
        }
    };

    match execute(&prog, &parsed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Stop::Code(code)) => ExitCode::from(code),
        Err(Stop::Fail(message)) => {
            eprintln!("Error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn execute(prog: &str, cmd: &Parsed) -> Outcome {
    if cmd.has("help") || cmd.is_empty() {
        eprint!("{}", text::help(prog, cmd.help_section().unwrap_or("")));
        return Ok(());
    }

    if cmd.has("fw-info") {
        let input = need(cmd, "in", "Input file not specified")?;
        let data = read_firmware(input)?;
        eprint!("{}", text::Container::probe(&data).describe());
        return Ok(());
    }

    if cmd.has("codeplug-info") {
        let input = need(cmd, "in", "Input file not specified")?;
        let data = std::fs::read(input).map_err(|_| format!("Cant open file: {input}"))?;
        eprint!("{}", text::codeplug(&data)?);
        return Ok(());
    }

    if cmd.has("wrap") {
        return wrap(cmd);
    }
    if cmd.has("unwrap") {
        return unwrap(cmd);
    }
    if cmd.has("make-xor") {
        return make_xor(cmd);
    }

    if cmd.has("list-serial-models") {
        for name in serial_models() {
            println!("{name}");
        }
        return Ok(());
    }

    if cmd.has("list-ble") {
        println!("Scanning...");
        for d in device::ble::scan(10, cmd.text("ble-adapter")).map_err(|e| e.to_string())? {
            if !d.name.is_empty() {
                println!("{}  {}", d.address, d.name);
            }
        }
        return Ok(());
    }

    if cmd.has("port") || cmd.has("ble") {
        return clone_cable(cmd);
    }

    if cmd.has("list") {
        for (ix, target) in targets()?.iter().enumerate() {
            println!("{}", target.describe(ix));
        }
        return Ok(());
    }

    let Some(index) = cmd.number("device") else {
        println!("You must select a device");
        return Err(Stop::Code(1));
    };

    let all = targets()?;
    let target = all.get(usize::from(index)).ok_or_else(|| {
        format!(
            "vector::_M_range_check: __n (which is {index}) >= this->size() (which is {})",
            all.len()
        )
    })?;

    on_target(cmd, target)
}

fn need<'a>(cmd: &'a Parsed, key: &str, missing: &str) -> Result<&'a str, Stop> {
    cmd.text(key).ok_or_else(|| Stop::from(missing))
}

fn read_firmware(path: &str) -> Result<Vec<u8>, Stop> {
    std::fs::read(path).map_err(|_| Stop::from("Can't open firmware file"))
}

fn wrap(cmd: &Parsed) -> Outcome {
    let out = need(cmd, "out", "Output file not specified")?;
    let radio = need(cmd, "radio", "Radio not specified")?;
    if cmd.segments().is_empty() {
        return Err("Must specify at least 1 segment".into());
    }
    if !supports_wrap(radio) {
        return Err("Firmware model not supported".into());
    }

    let mut loaded = Vec::with_capacity(cmd.segments().len());
    for seg in cmd.segments() {
        let (addr, file) = seg
            .split_once(':')
            .ok_or("Segments must be in the format '0x0000:filename.bin'")?;
        let address = parse_address(addr)?;
        eprintln!("Adding segment 0x{address:08x} from file {file}");
        let data = std::fs::read(file).map_err(|_| "Cant open file for segment")?;
        loaded.push((address, data));
    }

    let bytes = crate::build_firmware(radio, &loaded, true)?;
    std::fs::write(out, bytes).map_err(|e| format!("cannot write {out}: {e}"))?;
    eprintln!("Done!");
    Ok(())
}

fn supports_wrap(radio: &str) -> bool {
    firmware::tyt::config_for_model(radio).is_some()
        || firmware::cs::supports_model(radio)
        || firmware::sgl::config_for_model(radio).is_some()
        || firmware::yaesu::supports_model(radio)
        || firmware::ailunce::supports_model(radio)
}

fn parse_address(text: &str) -> Result<u32, Stop> {
    let base = if text.contains("0x") { 16 } else { 10 };
    let trimmed = text.trim_start();
    let (negative, unsigned) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let digits = if base == 16 {
        unsigned
            .strip_prefix("0x")
            .or_else(|| unsigned.strip_prefix("0X"))
            .unwrap_or(unsigned)
    } else {
        unsigned
    };
    let end = digits
        .find(|c: char| !c.is_digit(base))
        .unwrap_or(digits.len());
    let digits = digits.get(..end).unwrap_or_default();
    if digits.is_empty() {
        return Err("stoull".into());
    }
    let value = u64::from_str_radix(digits, base).map_err(|_| "stoull")?;
    let value = if negative {
        value.wrapping_neg()
    } else {
        value
    };
    u32::try_from(value).map_err(|_| Stop::Fail(format!("Segment address out of range: {text}")))
}

fn unwrap(cmd: &Parsed) -> Outcome {
    let input = need(cmd, "in", "Input file not specified")?;
    let out = need(cmd, "out", "Output file not specified")?;
    let data = read_firmware(input)?;

    for (address, segment) in text::Container::probe(&data).decrypted_regions() {
        let name = format!("{out}_0x{address:08x}");
        if std::fs::write(&name, segment).is_err() {
            eprintln!("Failed to open output file: {out}");
            return Err(Stop::Code(1));
        }
    }
    Ok(())
}

fn make_xor(cmd: &Parsed) -> Outcome {
    use firmware::keyguess;

    let input = need(cmd, "in", "Input file not specified")?;
    let data = read_firmware(input)?;
    let container = text::Container::probe(&data);

    let regions = container.stored_regions();
    let all: Vec<u8> = regions
        .iter()
        .flat_map(|(_, d)| d.iter().copied())
        .collect();
    let all = if all.is_empty() { container.raw() } else { all };
    let key = keyguess::guess_key(&all);

    for (address, region) in &regions {
        if verify(*address, region, &key) {
            println!("Region @ 0x{address:08x} appears to be a valid vector_table");
        }
    }

    eprintln!("{}", text::hex_dump(&key));
    let array: String = key.iter().map(|b| format!("0x{b:02x},")).collect();
    println!("const uint8_t[] key = {{{array}}};");
    Ok(())
}

fn verify(base: u32, region: &[u8], key: &[u8]) -> bool {
    const ENTRIES: usize = 0x61;
    if key.is_empty() || region.len() < ENTRIES * 4 {
        return false;
    }
    let plain: Vec<u8> = region
        .iter()
        .take(ENTRIES * 4)
        .zip(key.iter().cycle())
        .map(|(b, k)| b ^ k)
        .collect();
    let word = |ix: usize| -> [u8; 4] {
        let mut w = [0u8; 4];
        for (n, slot) in w.iter_mut().enumerate() {
            *slot = plain.get(ix * 4 + n).copied().unwrap_or(0);
        }
        w
    };

    if u32::from_le_bytes(word(0)) & 0x2ffe_0000 != 0x2000_0000 {
        eprintln!("Stack top invalid");
        return false;
    }

    let max = u64::from(base) + region.len() as u64;
    for ix in 1..ENTRIES {
        let irq = u32::from_le_bytes(word(ix));
        if irq != 0 && (irq <= base || u64::from(irq) >= max) {
            let bytes = |b: &[u8]| -> String {
                (0..4)
                    .map(|n| format!("0x{:02x}", b.get(ix * 4 + n).copied().unwrap_or(0)))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            eprintln!(
                "Invalid vector_table address: [{ix:x}]-0x{irq:08x}\n  Input: {}\n  Key:   {}\n  Out:   {}",
                bytes(region),
                bytes(key),
                bytes(&plain)
            );
            return false;
        }
    }
    true
}

fn serial_models() -> impl Iterator<Item = &'static str> {
    device::uv5r::ALL
        .iter()
        .map(|m| m.name)
        .chain(device::uv17pro::ALL.iter().map(|m| m.name))
}

fn clone_cable(cmd: &Parsed) -> Outcome {
    let radio = need(
        cmd,
        "radio",
        "Radio model not specified, see --list-serial-models",
    )?;

    if cmd.has("ble") {
        if device::uv17pro::model(radio).is_none() {
            let names: Vec<&str> = device::uv17pro::ALL.iter().map(|m| m.name).collect();
            return Err(Stop::Fail(format!(
                "Radio model '{radio}' does not clone over Bluetooth, supported models are: {}",
                names.join(" ")
            )));
        }
    } else if device::uv5r::model(radio).is_none() && device::uv17pro::model(radio).is_none() {
        let names: Vec<&str> = serial_models().collect();
        return Err(Stop::Fail(format!(
            "Unknown radio model '{radio}', supported models are: {}",
            names.join(" ")
        )));
    }

    let port = if cmd.has("ble") {
        None
    } else {
        cmd.text("port")
    };
    let ble = cmd.text("ble");
    let adapter = cmd.text("ble-adapter");

    if cmd.has("read-codeplug") {
        let out = need(cmd, "out", "Output file not specified")?;
        crate::read_codeplug(port, ble, adapter, radio, Path::new(out))?;
        println!("Done!");
        return Ok(());
    }

    if cmd.has("program") {
        let input = need(cmd, "in", "Input file not specified")?;
        if device::uv5r::model(radio).is_some() {
            return Err("Codeplug upload is not supported for this radio".into());
        }
        crate::write_codeplug(Path::new(input), port, ble, adapter, radio, true)?;
        println!("Done!");
        return Ok(());
    }

    if cmd.has("info") {
        println!("{}", cable_info(port, ble, adapter, radio)?);
        return Ok(());
    }

    println!("You must specify an operation, try --info or --read-codeplug");
    Err(Stop::Code(1))
}

fn cable_info(
    port: Option<&str>,
    ble: Option<&str>,
    adapter: Option<&str>,
    radio: &str,
) -> Result<String, Stop> {
    use device::{BoxedStream, ble::BleStream, serial::SerialStream};

    let timeout = Duration::from_millis(2000);
    let baud = if device::uv5r::model(radio).is_some() {
        9600
    } else {
        115_200
    };
    let stream: BoxedStream = match (ble, port) {
        (Some(address), _) => {
            Box::new(BleStream::connect(address, adapter, 15, timeout).map_err(|e| e.to_string())?)
        }
        (None, Some(port)) => {
            Box::new(SerialStream::open(port, baud, timeout).map_err(|e| e.to_string())?)
        }
        (None, None) => return Err("a serial port or a Bluetooth address is needed".into()),
    };

    if let Some(model) = device::uv5r::model(radio) {
        let info = device::uv5r::Session::new(stream, model)
            .info()
            .map_err(|e| e.to_string())?;
        let ident: String = info.ident.iter().map(|b| format!("{b:02x}")).collect();
        let firmware = if info.firmware.is_empty() {
            "Unknown"
        } else {
            &info.firmware
        };
        return Ok(format!(
            "Model:    Baofeng {}\nIdent:    {ident}\nFirmware: {firmware}",
            model.name
        ));
    }

    let model = device::uv17pro::model(radio).ok_or("unknown radio")?;
    let info = device::uv17pro::Session::new(stream, model)
        .info()
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "Model:  Baofeng {}\nIdent:  {}",
        model.name,
        info.describe()
    ))
}

enum Target {
    Usb(Found),
    Serial(String),
}

impl Target {
    fn describe(&self, index: usize) -> String {
        match self {
            Self::Usb(found) => {
                let (mfg, prd) = match found.known.protocol {
                    Protocol::H8sx => ("Yaesu".to_owned(), "FT-70D".to_owned()),
                    Protocol::TytHid => ("TYT".to_owned(), "SGL".to_owned()),
                    Protocol::TytDfu => (
                        found.manufacturer.clone().unwrap_or_default(),
                        found.product_name.clone().unwrap_or_default(),
                    ),
                };
                format!(
                    "[{:04x}:{:04x}]: idx={index:03}, {mfg} {prd}",
                    found.known.vendor, found.known.product
                )
            }
            Self::Serial(port) => format!("[{port}]: idx={index}, Generic serial radio"),
        }
    }
}

fn targets() -> Result<Vec<Target>, Stop> {
    let mut all: Vec<Target> = device::usb::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(Target::Usb)
        .collect();
    all.extend(serial_ports().into_iter().map(Target::Serial));
    Ok(all)
}

#[cfg(target_os = "linux")]
fn serial_ports() -> Vec<String> {
    let Ok(dir) = std::fs::read_dir("/dev/serial/by-id") else {
        return Vec::new();
    };
    dir.filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_symlink()))
        .filter_map(|e| std::fs::canonicalize(e.path()).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

#[cfg(target_os = "macos")]
fn serial_ports() -> Vec<String> {
    let Ok(dir) = std::fs::read_dir("/dev") else {
        return Vec::new();
    };
    dir.filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s == "tty" || s == "cu")
        })
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn serial_ports() -> Vec<String> {
    device::serial::SerialStream::list()
}

const TYT_ONLY: &[&str] = &[
    "get-status",
    "dump-reg",
    "dump-bootloader",
    "get-time",
    "set-time",
    "reboot",
];

fn on_target(cmd: &Parsed, target: &Target) -> Outcome {
    let found = match target {
        Target::Serial(port) => return on_ailunce(cmd, port),
        Target::Usb(found) => found,
    };
    let _ = crate::CHOSEN.set(found.clone());

    if cmd.has("info") {
        println!("{}", usb_info(found)?);
        return Ok(());
    }

    if TYT_ONLY.iter().any(|k| cmd.has(k)) {
        if found.known.protocol != Protocol::TytDfu {
            return Err("Cant use TYT commands on a non-TYT radio".into());
        }
        return tyt_commands(cmd, found);
    }

    if cmd.has("flash") {
        let input = need(cmd, "in", "Input file not specified")?;
        crate::flash(Path::new(input), true)?;
        println!("Done!");
        return Ok(());
    }

    if cmd.has("read-codeplug") {
        let out = need(cmd, "out", "Output file not specified")?;
        if found.known.protocol != Protocol::TytDfu {
            return Err("Codeplug download is not supported for this radio".into());
        }
        crate::usb_read_codeplug(Path::new(out))?;
        println!("Done!");
        return Ok(());
    }

    if cmd.has("program") {
        let input = need(cmd, "in", "Input file not specified")?;
        if found.known.protocol != Protocol::TytDfu {
            return Err("Codeplug upload is not supported for this radio".into());
        }
        crate::usb_write_codeplug(Path::new(input), true)?;
        println!("Done!");
        return Ok(());
    }

    Ok(())
}

fn on_ailunce(cmd: &Parsed, port: &str) -> Outcome {
    if cmd.has("info") {
        println!("== Ailunce USB Serial Cable ==");
        return Ok(());
    }
    if TYT_ONLY.iter().any(|k| cmd.has(k)) {
        return Err("Cant use TYT commands on a non-TYT radio".into());
    }
    if cmd.has("flash") {
        let input = need(cmd, "in", "Input file not specified")?;
        crate::flash_ailunce(&read_firmware(input)?, port, true)?;
        println!("Done!");
        return Ok(());
    }
    if cmd.has("read-codeplug") {
        need(cmd, "out", "Output file not specified")?;
        return Err("Codeplug download is not supported for this radio".into());
    }
    if cmd.has("program") {
        need(cmd, "in", "Input file not specified")?;
        return Err("Codeplug upload is not supported for this radio".into());
    }
    Ok(())
}

fn open(found: &Found, secs: u64) -> Result<UsbDevice, Stop> {
    Ok(UsbDevice::open_found(found, Duration::from_secs(secs)).map_err(|e| e.to_string())?)
}

fn usb_info(found: &Found) -> Result<String, Stop> {
    match found.known.protocol {
        Protocol::TytDfu => {
            let mut dfu = device::dfu::Dfu::new(open(found, 5)?);
            let model = device::dfu::tyt::identify(&mut dfu).map_err(|e| e.to_string())?;
            let rtc = clock_text(&mut dfu)?;
            Ok(format!("== TYT Radio Info ==\nRadio: {model}\nRTC: {rtc}"))
        }
        Protocol::TytHid => Ok("== TYT SGL Radio Info ==\nRadio: ASD\nRTC: 000".to_owned()),
        Protocol::H8sx => {
            let mut usb = open(found, 5)?;
            crate::claim_h8sx(&mut usb)?;
            let id = device::h8sx::identify(&mut usb).map_err(|e| e.to_string())?;
            Ok(format!("== Yaesu Radio Info ==\nRadio: {id}\n"))
        }
    }
}

fn clock_text<T: device::dfu::ControlTransfer>(
    dfu: &mut device::dfu::Dfu<T>,
) -> Result<String, Stop> {
    let (stamp, _) = device::dfu::tyt::read_clock(dfu).map_err(|e| e.to_string())?;
    let Some(t) = stamp else {
        return Ok("N/A".to_owned());
    };
    let when =
        chrono::NaiveDate::from_ymd_opt(i32::from(t.year), u32::from(t.month), u32::from(t.day))
            .and_then(|d| {
                d.and_hms_opt(u32::from(t.hour), u32::from(t.minute), u32::from(t.second))
            });
    Ok(match when {
        Some(when) => format!("{}\n", when.format("%a %b %e %H:%M:%S %Y")),
        None => "N/A".to_owned(),
    })
}

fn tyt_commands(cmd: &Parsed, found: &Found) -> Outcome {
    use device::dfu::{Dfu, tyt};

    if cmd.has("get-status") {
        let mut dfu = Dfu::new(open(found, 5)?);
        let s = dfu.status().map_err(|e| e.to_string())?;
        let t = s.poll_timeout;
        let shown = ((t & 0xff) << 16) | (t & 0xff00) | ((t >> 16) & 0xff);
        eprintln!(
            "Status: {}, Timeout: 0x{shown:02x}, State: {}, Discarded: 0x{:02x}",
            text::status_name(s.status),
            text::state_name(s.state),
            s.string_index
        );
    }

    if let Some(reg) = cmd.number("dump-reg") {
        eprintln!("Read register: 0x{reg:02x}");
        let mut dfu = Dfu::new(open(found, 5)?);
        dfu.download(&[tyt::REGISTER_COMMAND, reg as u8], 0)
            .map_err(|e| e.to_string())?;
        let data = dfu
            .upload(tyt::REGISTER_SIZE, 0)
            .map_err(|e| e.to_string())?;
        eprintln!("{}", text::hex_dump(&data));
    }

    if cmd.has("dump-bootloader") {
        let out = need(cmd, "out", "Output file not specified")?;
        crate::usb_dump_bootloader(Path::new(out))?;
    }

    if cmd.has("get-time") {
        let mut dfu = Dfu::new(open(found, 10)?);
        eprint!("{}", clock_text(&mut dfu)?.trim_end_matches('\n'));
        eprintln!();
    }

    if cmd.has("set-time") {
        crate::usb_set_time(None)?;
    }

    if cmd.has("reboot") {
        let mut dfu = Dfu::new(open(found, 5)?);
        tyt::reboot(&mut dfu).map_err(|e| e.to_string())?;
    }

    Ok(())
}
