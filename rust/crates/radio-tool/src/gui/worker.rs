use device::usb::{Found, Protocol, UsbDevice};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum Radio {
    Usb(Found),
    Cable {
        port: String,
        model: &'static str,
    },
    Bluetooth {
        address: String,
        name: String,
        model: &'static str,
    },
}

impl Radio {
    pub fn key(&self) -> String {
        match self {
            Radio::Usb(f) => format!("usb:{}:{}", f.bus, f.address),
            Radio::Cable { port, .. } => format!("port:{port}"),
            Radio::Bluetooth { address, .. } => format!("ble:{address}"),
        }
    }

    pub fn title(&self) -> String {
        match self {
            Radio::Usb(f) => f
                .product_name
                .clone()
                .unwrap_or_else(|| f.known.description.to_owned()),
            Radio::Cable { model, .. } => format!("Baofeng {model}"),
            Radio::Bluetooth { name, .. } => name.clone(),
        }
    }

    pub fn kind(&self) -> String {
        match self {
            Radio::Usb(f) => match f.known.protocol {
                Protocol::TytDfu => "usb / tyt dfu".into(),
                Protocol::TytHid => "usb / sgl bootloader".into(),
                Protocol::H8sx => "usb / yaesu bootloader".into(),
            },
            Radio::Cable { .. } => "serial cable".into(),
            Radio::Bluetooth { .. } => "bluetooth".into(),
        }
    }

    pub fn place(&self) -> String {
        match self {
            Radio::Usb(f) => format!(
                "{:04x}:{:04x} bus {} addr {}",
                f.known.vendor, f.known.product, f.bus, f.address
            ),
            Radio::Cable { port, .. } => port.clone(),
            Radio::Bluetooth { address, .. } => address.clone(),
        }
    }

    pub fn reads_codeplug(&self) -> bool {
        match self {
            Radio::Usb(f) => f.known.protocol == Protocol::TytDfu,
            Radio::Cable { .. } | Radio::Bluetooth { .. } => true,
        }
    }

    pub fn takes_firmware(&self) -> bool {
        matches!(self, Radio::Usb(_))
    }

    pub fn has_clock(&self) -> bool {
        matches!(self, Radio::Usb(f) if f.known.protocol == Protocol::TytDfu)
    }
}

#[derive(Clone, Debug)]
pub enum Job {
    Scan,
    ScanBluetooth,
    Identify(Radio),
    ReadCodeplug(Radio, PathBuf),
    WriteCodeplug(Radio, PathBuf),
    Flash(Radio, PathBuf),
    ReadClock(Radio),
    SetClock(Radio),
    Reboot(Radio),
}

impl Job {
    pub fn label(&self) -> &'static str {
        match self {
            Job::Scan => "scanning",
            Job::ScanBluetooth => "scanning bluetooth",
            Job::Identify(_) => "asking the radio",
            Job::ReadCodeplug(..) => "reading the codeplug",
            Job::WriteCodeplug(..) => "writing the codeplug",
            Job::Flash(..) => "writing firmware",
            Job::ReadClock(_) => "reading the clock",
            Job::SetClock(_) => "setting the clock",
            Job::Reboot(_) => "restarting",
        }
    }
}

pub enum Event {
    Found(Vec<Radio>),
    FoundBluetooth(Vec<(String, String)>),
    Progress {
        done: usize,
        total: usize,
        what: String,
    },
    Identified(Vec<(String, String)>),
    Saved(PathBuf, Vec<u8>),
    Clock(String),
    Done(String),
    Failed(String),
}

pub struct Worker {
    tx: Sender<Job>,
    pub rx: Receiver<Event>,
}

impl Worker {
    pub fn spawn(ctx: egui::Context) -> Result<Self, String> {
        let (tx, jobs) = channel::<Job>();
        let (out, rx) = channel();
        std::thread::Builder::new()
            .name("radio".into())
            .spawn(move || {
                for job in jobs {
                    let send = |e: Event| {
                        let _ = out.send(e);
                        ctx.request_repaint();
                    };
                    let result = run(job, &send);
                    send(match result {
                        Ok(done) => done,
                        Err(e) => Event::Failed(e),
                    });
                }
            })
            .map_err(|e| format!("cannot start the radio thread: {e}"))?;
        Ok(Self { tx, rx })
    }

    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}

type Send<'a> = &'a dyn Fn(Event);

fn progress(send: Send, done: usize, total: usize, what: &str) {
    send(Event::Progress {
        done,
        total,
        what: what.to_owned(),
    });
}

fn run(job: Job, send: Send) -> Result<Event, String> {
    match job {
        Job::Scan => Ok(Event::Found(scan()?)),
        Job::ScanBluetooth => {
            let found = device::ble::scan(8, None).map_err(|e| e.to_string())?;
            Ok(Event::FoundBluetooth(
                found
                    .into_iter()
                    .filter(|d| !d.name.is_empty())
                    .map(|d| (d.address, d.name))
                    .collect(),
            ))
        }
        Job::Identify(radio) => Ok(Event::Identified(identify(&radio)?)),
        Job::ReadCodeplug(radio, path) => {
            let image = read_codeplug(&radio, send)?;
            std::fs::write(&path, &image)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            Ok(Event::Saved(path, image))
        }
        Job::WriteCodeplug(radio, path) => {
            let data =
                std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            write_codeplug(&radio, &data, send)?;
            Ok(Event::Done("codeplug written, restart the radio".into()))
        }
        Job::Flash(radio, path) => {
            let data =
                std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            flash(&radio, &data, send)?;
            Ok(Event::Done("firmware written, restart the radio".into()))
        }
        Job::ReadClock(radio) => Ok(Event::Clock(read_clock(&radio)?)),
        Job::SetClock(radio) => {
            set_clock(&radio)?;
            Ok(Event::Clock(read_clock(&radio)?))
        }
        Job::Reboot(radio) => {
            let mut dfu = device::dfu::Dfu::new(open(&radio, 5)?);
            device::dfu::tyt::reboot(&mut dfu).map_err(|e| e.to_string())?;
            Ok(Event::Done(
                "restart sent. A radio in bootloader mode ignores it".into(),
            ))
        }
    }
}

pub fn serial_ports() -> Vec<String> {
    device::serial::SerialStream::list()
}

fn scan() -> Result<Vec<Radio>, String> {
    Ok(device::usb::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(Radio::Usb)
        .collect())
}

fn usb(radio: &Radio) -> Result<&Found, String> {
    match radio {
        Radio::Usb(f) => Ok(f),
        _ => Err("this is not a USB radio".into()),
    }
}

fn open(radio: &Radio, secs: u64) -> Result<UsbDevice, String> {
    UsbDevice::open_found(usb(radio)?, Duration::from_secs(secs)).map_err(|e| e.to_string())
}

fn stream(radio: &Radio, baud: u32) -> Result<device::BoxedStream, String> {
    let timeout = Duration::from_millis(2000);
    match radio {
        Radio::Cable { port, .. } => Ok(Box::new(
            device::serial::SerialStream::open(port, baud, timeout).map_err(|e| e.to_string())?,
        )),
        Radio::Bluetooth { address, .. } => Ok(Box::new(
            device::ble::BleStream::connect(address, None, 15, timeout)
                .map_err(|e| e.to_string())?,
        )),
        Radio::Usb(_) => Err("this radio is on USB".into()),
    }
}

fn model_of(radio: &Radio) -> Option<&'static str> {
    match radio {
        Radio::Cable { model, .. } | Radio::Bluetooth { model, .. } => Some(model),
        Radio::Usb(_) => None,
    }
}

fn identify(radio: &Radio) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    match radio {
        Radio::Usb(f) => match f.known.protocol {
            Protocol::TytDfu => {
                let usb = open(radio, 5)?;
                if let Ok(layout) = usb.memory_layout() {
                    for m in layout {
                        out.push((format!("memory {}", m.alt), m.to_string()));
                    }
                }
                let mut dfu = device::dfu::Dfu::new(usb);
                out.insert(
                    0,
                    (
                        "model".into(),
                        device::dfu::tyt::identify(&mut dfu).map_err(|e| e.to_string())?,
                    ),
                );
                if let Ok(state) = dfu.state() {
                    out.push(("dfu state".into(), format!("{state:?}")));
                }
            }
            Protocol::H8sx => {
                let mut usb = open(radio, 5)?;
                usb.claim_with_detach(0).map_err(|e| e.to_string())?;
                usb.set_endpoints(0x01, 0x82);
                let id = device::h8sx::identify(&mut usb).map_err(|e| e.to_string())?;
                out.push(("device".into(), id.to_string()));
            }
            Protocol::TytHid => {
                out.push(("mode".into(), "firmware update bootloader".into()));
                out.push((
                    "note".into(),
                    "this bootloader takes firmware and says nothing about itself".into(),
                ));
            }
        },
        Radio::Cable { model, .. } | Radio::Bluetooth { model, .. } => {
            if let Some(m) = device::uv5r::model(model) {
                let info = device::uv5r::Session::new(stream(radio, 9600)?, m)
                    .info()
                    .map_err(|e| e.to_string())?;
                out.push(("model".into(), format!("Baofeng {model}")));
                out.push((
                    "ident".into(),
                    info.ident.iter().map(|b| format!("{b:02x}")).collect(),
                ));
                out.push((
                    "firmware".into(),
                    if info.firmware.is_empty() {
                        "unknown".into()
                    } else {
                        info.firmware
                    },
                ));
            } else if let Some(m) = device::uv17pro::model(model) {
                let info = device::uv17pro::Session::new(stream(radio, 115_200)?, m)
                    .info()
                    .map_err(|e| e.to_string())?;
                out.push(("model".into(), format!("Baofeng {model}")));
                out.push(("ident".into(), info.describe()));
            }
        }
    }
    Ok(out)
}

fn read_codeplug(radio: &Radio, send: Send) -> Result<Vec<u8>, String> {
    match radio {
        Radio::Usb(_) => {
            let mut usb = open(radio, 30)?;
            usb.claim(0).map_err(|e| e.to_string())?;
            let mut dfu = device::dfu::Dfu::new(usb);
            device::dfu::codeplug::read(&mut dfu, |p| progress(send, p.done, p.total, "read"))
                .map_err(|e| e.to_string())
        }
        _ => {
            let model = model_of(radio).ok_or("no model chosen")?;
            if let Some(m) = device::uv5r::model(model) {
                progress(send, 0, 0, "read");
                let (image, _) = device::uv5r::Session::new(stream(radio, 9600)?, m)
                    .download()
                    .map_err(|e| e.to_string())?;
                Ok(image)
            } else {
                let m = device::uv17pro::model(model).ok_or("unknown model")?;
                progress(send, 0, 0, "read");
                let (image, _) = device::uv17pro::Session::new(stream(radio, 115_200)?, m)
                    .download()
                    .map_err(|e| e.to_string())?;
                Ok(image)
            }
        }
    }
}

fn write_codeplug(radio: &Radio, data: &[u8], send: Send) -> Result<(), String> {
    match radio {
        Radio::Usb(_) => {
            if data.len() != device::dfu::codeplug::MEMORY_SIZE {
                return Err(format!(
                    "this file is {} bytes, and a codeplug image for this radio is {}",
                    data.len(),
                    device::dfu::codeplug::MEMORY_SIZE
                ));
            }
            let mut usb = open(radio, 30)?;
            usb.claim(0).map_err(|e| e.to_string())?;
            let mut dfu = device::dfu::Dfu::new(usb);
            device::dfu::codeplug::write(&mut dfu, data, |p| {
                progress(send, p.done, p.total, "write")
            })
            .map_err(|e| e.to_string())
        }
        _ => {
            let model = model_of(radio).ok_or("no model chosen")?;
            if let Some(m) = device::uv5r::model(model) {
                let image = codeplug::uv5r::strip_chirp_metadata(data);
                device::uv5r::Session::new(stream(radio, 9600)?, m)
                    .upload(image, |d, t| progress(send, d, t, "write"))
                    .map_err(|e| e.to_string())
            } else {
                let m = device::uv17pro::model(model).ok_or("unknown model")?;
                device::uv17pro::Session::new(stream(radio, 115_200)?, m)
                    .upload(data, |d, t| progress(send, d, t, "write"))
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    }
}

fn flash(radio: &Radio, data: &[u8], send: Send) -> Result<(), String> {
    use device::dfu::{Dfu, flashing};

    let found = usb(radio)?;
    match found.known.protocol {
        Protocol::H8sx => {
            let fw = firmware::yaesu::YaesuFirmware::parse(data).map_err(|e| e.to_string())?;
            let mut dev = open(radio, 30)?;
            dev.claim_with_detach(0).map_err(|e| e.to_string())?;
            dev.set_endpoints(0x01, 0x82);
            let check = device::h8sx::flash(&mut dev, fw.data(), |p| {
                progress(send, p.sent, p.total, "write")
            })
            .map_err(|e| e.to_string())?;
            if check.agrees() {
                Ok(())
            } else {
                Err(format!("the radio's sum check failed: {}", check.explain()))
            }
        }
        Protocol::TytHid => {
            let fw = crate::format::Firmware::identify(data)
                .ok_or("this is not a firmware file this tool reads")?;
            let identity = fw
                .sgl_identity()
                .ok_or("this radio takes SGL firmware, and this file is not one")?;
            let stored = fw.segments_as_stored()?;
            let body = stored
                .first()
                .map(|(_, d)| d.clone())
                .ok_or("this firmware is empty")?;
            let mut dev = open(radio, 30)?;
            dev.claim_hid().map_err(|e| e.to_string())?;
            let mut session = device::hid::Session::new(dev);
            device::hid::flashing::write(&mut session, &identity, &body, |p| {
                progress(send, p.written, p.total, "write")
            })
            .map_err(|e| e.to_string())
        }
        Protocol::TytDfu => {
            let fw = crate::format::Firmware::identify(data)
                .ok_or("this is not a firmware file this tool reads")?;
            if !matches!(fw, crate::format::Firmware::Tyt(_)) {
                return Err(format!(
                    "this is {} firmware, and the radio takes TYT firmware",
                    fw.radio()
                ));
            }
            let dev = open(radio, 30)?;
            let layout = dev.memory_layout().map_err(|e| e.to_string())?;
            let map = layout
                .iter()
                .find(|m| m.alt == 0)
                .ok_or("the radio did not describe its internal flash")?
                .programmable();
            let stored = fw.segments_as_stored()?;
            let regions: Vec<flashing::Region<'_>> = stored
                .iter()
                .map(|(address, data)| flashing::Region {
                    address: *address,
                    data,
                })
                .filter(|r| {
                    let end = r.address + r.data.len() as u32;
                    device::flash::split(&map, r.address, end)
                        .iter()
                        .map(|p| p.length)
                        .sum::<u32>()
                        == end.saturating_sub(r.address)
                })
                .collect();
            if regions.is_empty() {
                return Err("none of this firmware falls in memory the radio will write".into());
            }
            let mut dfu = Dfu::new(dev);
            flashing::write(&mut dfu, &regions, &map, |step| match step {
                flashing::Step::Erasing { .. } => progress(send, 0, 0, "erase"),
                flashing::Step::Writing { written, total, .. } => {
                    progress(send, written, total, "write")
                }
            })
            .map_err(|e| e.to_string())
        }
    }
}

fn read_clock(radio: &Radio) -> Result<String, String> {
    let mut dfu = device::dfu::Dfu::new(open(radio, 10)?);
    let (stamp, _) = device::dfu::tyt::read_clock(&mut dfu).map_err(|e| e.to_string())?;
    Ok(stamp.map_or_else(
        || "this radio has no clock in that register".into(),
        |t| t.to_string(),
    ))
}

fn set_clock(radio: &Radio) -> Result<(), String> {
    use chrono::{Datelike, Local, Timelike};
    let now = Local::now().naive_local();
    let stamp = device::dfu::tyt::Timestamp {
        year: u16::try_from(now.year()).map_err(|_| "the year is out of range")?,
        month: now.month() as u8,
        day: now.day() as u8,
        hour: now.hour() as u8,
        minute: now.minute() as u8,
        second: now.second() as u8,
    };
    let mut dfu = device::dfu::Dfu::new(open(radio, 10)?);
    device::dfu::tyt::write_clock(&mut dfu, stamp).map_err(|e| e.to_string())
}

pub struct ChannelRow {
    pub slot: usize,
    pub name: String,
    pub rx: String,
    pub tx: String,
    pub rx_tone: String,
    pub tx_tone: String,
    pub mode: String,
    pub power: String,
    pub scan: bool,
}

pub enum FileView {
    Firmware {
        kind: String,
        facts: Vec<(String, String)>,
        segments: Vec<(u32, usize)>,
    },
    Codeplug {
        kind: String,
        facts: Vec<(String, String)>,
        channels: Vec<ChannelRow>,
        slots: usize,
    },
}

impl FileView {
    pub fn summary(&self) -> String {
        match self {
            FileView::Firmware { kind, facts, .. } | FileView::Codeplug { kind, facts, .. } => {
                let mut out = kind.clone();
                for (k, v) in facts.iter().take(4) {
                    out.push_str(&format!("\n{k}: {v}"));
                }
                out
            }
        }
    }
}

fn mhz(hz: u32) -> String {
    format!("{:.5}", f64::from(hz) / 1_000_000.0)
}

pub fn describe_file(path: &Path) -> Result<FileView, String> {
    let data = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    if let Some(fw) = crate::format::Firmware::identify(&data) {
        let text = fw.describe();
        let mut lines = text.lines();
        let kind = lines
            .next()
            .unwrap_or_default()
            .trim_matches(|c| c == '=' || c == ' ')
            .to_owned();
        let facts = lines
            .take_while(|l| !l.starts_with("Data Segments"))
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_owned()))
            .collect();
        let segments = fw
            .segments()
            .iter()
            .map(|s| (s.address, s.data.len()))
            .collect();
        return Ok(FileView::Firmware {
            kind,
            facts,
            segments,
        });
    }

    let stripped = codeplug::uv5r::strip_chirp_metadata(&data);
    if codeplug::uv5r::Uv5rCodeplug::is_supported(&data) {
        let cp = codeplug::uv5r::Uv5rCodeplug::parse(stripped).map_err(|e| e.to_string())?;
        let channels = cp
            .used_channels()
            .map(|(slot, c)| ChannelRow {
                slot,
                name: c.name.clone(),
                rx: mhz(c.rx_freq),
                tx: if c.tx_inhibit {
                    "off".into()
                } else {
                    mhz(c.tx_freq)
                },
                rx_tone: c.rx_tone.to_string(),
                tx_tone: c.tx_tone.to_string(),
                mode: if c.wide {
                    "Wide".into()
                } else {
                    "Narrow".into()
                },
                power: c.power.to_string(),
                scan: c.scan,
            })
            .collect();
        let facts = vec![
            ("firmware".into(), or_unknown(cp.firmware_version())),
            ("power on".into(), or_unknown(cp.power_on_message())),
        ];
        return Ok(FileView::Codeplug {
            kind: "Baofeng UV-5R codeplug".into(),
            facts,
            channels,
            slots: cp.channels().len(),
        });
    }

    if codeplug::uv17pro::Uv17ProCodeplug::is_supported(&data) {
        let cp = codeplug::uv17pro::Uv17ProCodeplug::parse(&data).map_err(|e| e.to_string())?;
        let channels = cp
            .used_channels()
            .map(|(slot, c)| ChannelRow {
                slot,
                name: c.name.clone(),
                rx: mhz(c.rx_freq),
                tx: mhz(c.tx_freq),
                rx_tone: c.rx_tone.to_string(),
                tx_tone: c.tx_tone.to_string(),
                mode: c.mode.to_string(),
                power: c.power.to_string(),
                scan: c.scan,
            })
            .collect();
        return Ok(FileView::Codeplug {
            kind: format!("Baofeng {} codeplug", cp.model().name),
            facts: Vec::new(),
            channels,
            slots: cp.channels().len(),
        });
    }

    if codeplug::rdt::RdtCodeplug::is_supported(&data) {
        let cp = codeplug::rdt::RdtCodeplug::parse(&data).map_err(|e| e.to_string())?;
        let g = cp.general();
        let facts = vec![
            ("radio".into(), cp.radio().to_owned()),
            ("target".into(), cp.target_name().to_owned()),
            (
                "created".into(),
                cp.timestamp().map_or("invalid".into(), |t| t.to_string()),
            ),
            ("name".into(), g.radio_name.clone()),
            ("radio id".into(), g.radio_id.to_string()),
            (
                "intro".into(),
                format!("{} / {}", g.intro_line1, g.intro_line2),
            ),
        ];
        return Ok(FileView::Codeplug {
            kind: "TYT RDT codeplug".into(),
            facts,
            channels: Vec::new(),
            slots: 0,
        });
    }

    Err(
        "not a firmware or codeplug file this tool reads. A Yaesu or Ailunce firmware has no \
         header, so it cannot be recognised by looking at it"
            .into(),
    )
}

fn or_unknown(s: &str) -> String {
    if s.is_empty() {
        "unknown".into()
    } else {
        s.to_owned()
    }
}
