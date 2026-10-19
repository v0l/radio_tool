use crate::format::format_bytes;
use firmware::{cs, sgl, tyt};

const SECTIONS: &[(&str, &str)] = &[
    ("General", include_str!("help/general.txt")),
    ("Programming", include_str!("help/programming.txt")),
    ("Firmware", include_str!("help/firmware.txt")),
    ("All radio", include_str!("help/all_radio.txt")),
    ("TYT Radio", include_str!("help/tyt_radio.txt")),
    ("Serial radio", include_str!("help/serial_radio.txt")),
    ("Codeplug", include_str!("help/codeplug.txt")),
    ("Wrap", include_str!("help/wrap.txt")),
];

pub fn help(prog: &str, section: &str) -> String {
    let body = if section.is_empty() {
        include_str!("help/default.txt")
    } else {
        SECTIONS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(section))
            .map_or(include_str!("help/all.txt"), |(_, text)| text)
    };
    body.replace(
        "@VERSION@",
        &format!("radio_tool v{}", env!("CARGO_PKG_VERSION")),
    )
    .replace("@PROG@", prog)
    .to_owned()
}

pub enum Container {
    Tyt(Box<tyt::TytFirmware>, String),
    Cs(Box<cs::CsFirmware>),
    Sgl(Box<sgl::SglFirmware>),
    Headerless(Vec<u8>),
}

impl Container {
    pub fn probe(data: &[u8]) -> Self {
        if tyt::TytFirmware::is_supported(data)
            && let Ok(fw) = tyt::TytFirmware::parse(data)
        {
            let field = data.get(0x10..0x20).unwrap_or_default();
            let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
            let model = String::from_utf8_lossy(field.get(..end).unwrap_or_default());
            return Self::Tyt(Box::new(fw), model.into_owned());
        }
        if cs::CsFirmware::is_supported(data)
            && let Ok(fw) = cs::CsFirmware::parse(data)
        {
            return Self::Cs(Box::new(fw));
        }
        if sgl::SglFirmware::is_supported(data)
            && let Ok(fw) = sgl::SglFirmware::parse(data)
        {
            return Self::Sgl(Box::new(fw));
        }
        eprintln!(
            "{}",
            if data.len() < 0x10 {
                "File is too small to hold an SGL header"
            } else {
                "Invalid SGL header magic"
            }
        );
        let mut raw = data.to_vec();
        raw.resize(raw.len().next_multiple_of(1024), 0xff);
        Self::Headerless(raw)
    }

    pub fn raw(&self) -> Vec<u8> {
        match self {
            Self::Headerless(raw) => raw.clone(),
            _ => Vec::new(),
        }
    }

    pub fn decrypted_regions(&self) -> Vec<(u32, Vec<u8>)> {
        match self {
            Self::Tyt(fw, _) => fw
                .segments()
                .iter()
                .map(|s| (s.address, s.data.to_vec()))
                .collect(),
            Self::Cs(fw) => vec![(fw.base_address(), fw.data().to_vec())],
            Self::Sgl(fw) => vec![(0, fw.data().to_vec())],
            Self::Headerless(_) => Vec::new(),
        }
    }

    pub fn stored_regions(&self) -> Vec<(u32, Vec<u8>)> {
        match self {
            Self::Tyt(fw, _) => fw.segments_as_stored(),
            Self::Cs(fw) => {
                let mut data = fw.data().to_vec();
                firmware::cipher::apply_xor(&mut data, firmware::cipher::CS800_0, 0);
                vec![(fw.base_address(), data)]
            }
            Self::Sgl(fw) => vec![(0, fw.data_as_stored())],
            Self::Headerless(_) => Vec::new(),
        }
    }

    pub fn describe(&self) -> String {
        let segments = |out: &mut String, regions: &[(u32, Vec<u8>)]| {
            for (ix, (address, data)) in regions.iter().enumerate() {
                out.push_str(&format!(
                    "  {ix}: Start=0x{address:08x}, Length=0x{:08x}\n",
                    data.len()
                ));
            }
        };

        let mut out = String::new();
        match self {
            Self::Tyt(fw, model) => {
                let size: usize = fw.data().len();
                out.push_str("== TYT Firmware == \n");
                out.push_str(&format!("Radio: {model} ({})\n", fw.config().radio_model));
                out.push_str(&format!("Size:  {}\n", format_bytes(size as u64)));
                out.push_str("Data Segments: \n");
                segments(&mut out, &self.decrypted_regions());
            }
            Self::Cs(fw) => {
                out.push_str("== Connect Systems Firmware ==\n");
                out.push_str(&format!(
                    "Image Size: {}\n",
                    format_bytes(fw.data().len() as u64)
                ));
                out.push_str(&format!("Version:    {}\n", fw.version()));
                out.push_str(&format!(
                    "Checksum:   0x{:04x}\n",
                    fw.checksum().unwrap_or_default()
                ));
                out.push_str("Data Segments: \n");
                segments(&mut out, &self.decrypted_regions());
            }
            Self::Sgl(fw) => {
                let config = fw.config();
                let key = fw
                    .model_key()
                    .map(|k| String::from_utf8_lossy(&k).into_owned())
                    .unwrap_or_default();
                let offset = fw.secrets().map_or(0, |s| s.binary_offset);
                out.push_str("== TYT SGL Firmware == \n");
                out.push_str(&format!("Radio: {}\n", config.radio_model));
                out.push_str("SGL version: 1\n");
                out.push_str(&format!("Length: {}\n", fw.data().len()));
                out.push_str(&format!("Radio Group: {}\n", config.radio_group));
                out.push_str(&format!("Model: {}\n", config.header_model));
                out.push_str(&format!("Protocol Version: {}\n", config.protocol_version));
                out.push_str(&format!("Key: {key}\n"));
                out.push_str(&format!("Binary Offset: 0x400 + 0x{offset:02x}\n"));
                out.push_str("Data Segments: \n");
                out.push_str(&format!("  0: Length=0x{:08x}\n", fw.data().len()));
            }
            Self::Headerless(raw) => {
                out.push_str("== Yaesu Firmware == \n");
                out.push_str(&format!("Size:  {}\n", format_bytes(raw.len() as u64)));
            }
        }
        out
    }
}

pub fn codeplug(data: &[u8]) -> Result<String, String> {
    use codeplug::{rdt::RdtCodeplug, uv5r::Uv5rCodeplug};

    if RdtCodeplug::is_supported(data) {
        let cp = RdtCodeplug::parse(data).map_err(|e| e.to_string())?;
        let created = cp
            .timestamp()
            .and_then(|t| {
                chrono::NaiveDate::from_ymd_opt(
                    i32::from(t.year),
                    u32::from(t.month),
                    u32::from(t.day),
                )?
                .and_hms_opt(
                    u32::from(t.hour),
                    u32::from(t.minute),
                    u32::from(t.second),
                )
            })
            .map_or("Invalid".to_owned(), |t| {
                t.format("%a %b %e %H:%M:%S %Y").to_string()
            });
        let g = cp.general();
        return Ok(format!(
            " == RDT Codeplug ==\nRadio:   {}\nCreated: {created}\nTarget:  {}\nName:    {}\nRadio ID: {}\nIntro 1: {}\nIntro 2: {}\n",
            cp.radio(),
            cp.target_name(),
            g.radio_name,
            g.radio_id,
            g.intro_line1,
            g.intro_line2
        ));
    }

    if Uv5rCodeplug::is_supported(data) {
        let cp = Uv5rCodeplug::parse(data).map_err(|e| e.to_string())?;
        return Ok(uv5r_text(&cp));
    }

    let cp = codeplug::parse(data).map_err(|_| "Codeplug file not supported".to_owned())?;
    Ok(format!("{cp}\n"))
}

fn uv5r_text(cp: &codeplug::uv5r::Uv5rCodeplug) -> String {
    let or = |s: &str, fallback: &str| {
        if s.is_empty() {
            fallback.to_owned()
        } else {
            s.to_owned()
        }
    };
    let freq = |hz: u32| format!("{:.5}", f64::from(hz) / 1_000_000.0);

    let mut out = format!(
        " == Baofeng UV-5R Codeplug ==\nFirmware: {}\nPower on: {}\n\n  # Name       RX Freq    TX Freq    RX Tone  TX Tone  BW    Pwr   Scan\n",
        or(cp.firmware_version(), "Unknown"),
        or(cp.power_on_message(), "(none)")
    );
    let mut used = 0;
    for (ix, ch) in cp.used_channels() {
        used += 1;
        let tx = if ch.tx_inhibit {
            "off".to_owned()
        } else {
            freq(ch.tx_freq)
        };
        out.push_str(&format!(
            "{ix:3} {:<10} {:>10} {tx:>10} {:>8} {:>8} {:>5} {:>5} {}\n",
            ch.name,
            freq(ch.rx_freq),
            ch.rx_tone.to_string(),
            ch.tx_tone.to_string(),
            if ch.wide { "Wide" } else { "Narrow" },
            ch.power.to_string(),
            if ch.scan { "Yes" } else { "No" }
        ));
    }
    out.push_str(&format!(
        "\n{used} of {} channels used\n",
        cp.channels().len()
    ));
    out
}

pub fn hex_dump(data: &[u8]) -> String {
    let mut out = String::new();
    let mut ascii = String::new();
    let mut col = 0;
    for b in data {
        out.push_str(&format!("{b:02x} "));
        ascii.push(if (32..=126).contains(b) {
            char::from(*b)
        } else {
            '.'
        });
        col += 1;
        if col == 16 {
            out.push(' ');
            out.push_str(&ascii);
            out.push('\n');
            ascii.clear();
            col = 0;
        } else if col % 4 == 0 {
            out.push_str("  ");
        }
    }
    if col != 0 {
        for pad in col..16 {
            out.push_str("   ");
            if (pad + 1) % 4 == 0 && pad + 1 != 16 {
                out.push_str("  ");
            }
        }
        out.push(' ');
        out.push_str(&ascii);
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

pub fn status_name(status: u8) -> &'static str {
    const NAMES: [&str; 16] = [
        "OK",
        "errTARGET",
        "errFILE",
        "errWRITE",
        "errERASE",
        "errCHECK_ERASE",
        "errPROG",
        "errVERIFY",
        "errADDRESS",
        "errNOTDONE",
        "errFIRMWARE",
        "errVENDOR",
        "errUSBR",
        "errPOR",
        "errUNKNOWN",
        "errSTALLEDPKT",
    ];
    NAMES
        .get(usize::from(status))
        .copied()
        .unwrap_or("**UKNOWN**")
}

pub fn state_name(state: device::dfu::State) -> &'static str {
    use device::dfu::State;
    match state {
        State::AppIdle => "IDLE",
        State::AppDetach => "DETACH",
        State::Idle => "DFU_IDLE",
        State::DownloadSync => "DFU_DOWNLOAD_SYNC",
        State::DownloadBusy => "DFU_DOWNLOAD_BUSY",
        State::DownloadIdle => "DFU_DOWNLOAD_IDLE",
        State::ManifestSync => "DFU_MANIFEST_SYNC",
        State::Manifest => "DFU_MANIFEST",
        State::ManifestWaitReset => "DFU_MANIFEST_WAIT_RESET",
        State::UploadIdle => "DFU_UPLOAD_IDLE",
        State::Error => "DFU_ERROR",
        State::Unknown(0x91) => "DFU_UPLOAD_SYNC",
        State::Unknown(0x92) => "DFU_UPLOAD_BUSY",
        State::Unknown(_) => "**UKNOWN**",
    }
}
