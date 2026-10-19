mod worker;

use egui::{RichText, Ui};
use egui_bench::prelude::{
    Browser, Key, Line, Picked, Table, card, cell, chip, choice, hint, legend, note, progress,
    reading, theme,
};
use std::path::{Path, PathBuf};
use worker::{Event, FileView, Job, Radio, Worker};

const SIDE_W: f32 = 300.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    WriteCodeplug,
    Flash,
    SetClock,
    Reboot,
}

enum Pick {
    OpenFile,
    SaveCodeplug,
    WriteCodeplug,
    Flash,
}

struct Busy {
    what: &'static str,
    step: String,
    done: usize,
    total: usize,
}

struct App {
    worker: Worker,
    radios: Vec<Radio>,
    bluetooth: Vec<(String, String)>,
    ports: Vec<String>,
    selected: Option<String>,
    cable_port: String,
    cable_model: &'static str,
    ble_model: &'static str,
    facts: Vec<(String, String)>,
    clock: Option<String>,
    busy: Option<Busy>,
    message: Option<(String, bool)>,
    confirm: Option<(Ask, Option<PathBuf>)>,
    browser: Option<(Pick, Browser)>,
    folder: PathBuf,
    file: Option<(PathBuf, Result<FileView, String>)>,
}

fn cable_models() -> Vec<&'static str> {
    device::uv5r::ALL
        .iter()
        .map(|m| m.name)
        .chain(device::uv17pro::ALL.iter().map(|m| m.name))
        .collect()
}

fn ble_models() -> Vec<&'static str> {
    device::uv17pro::ALL.iter().map(|m| m.name).collect()
}

impl App {
    fn new(ctx: &egui::Context, open: Option<PathBuf>) -> Result<Self, String> {
        let worker = Worker::spawn(ctx.clone())?;
        worker.send(Job::Scan);
        let ports = worker::serial_ports();
        let folder = open
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let mut app = Self {
            worker,
            radios: Vec::new(),
            bluetooth: Vec::new(),
            cable_port: ports.first().cloned().unwrap_or_default(),
            ports,
            selected: None,
            cable_model: "UV5R",
            ble_model: "UV5RMINI",
            facts: Vec::new(),
            clock: None,
            busy: Some(Busy {
                what: "scanning",
                step: String::new(),
                done: 0,
                total: 0,
            }),
            message: None,
            confirm: None,
            browser: None,
            folder,
            file: None,
        };
        if let Some(path) = open {
            app.inspect(path);
        }
        Ok(app)
    }

    fn radio(&self) -> Option<Radio> {
        let key = self.selected.as_ref()?;
        if let Some(r) = self.radios.iter().find(|r| &r.key() == key) {
            return Some(r.clone());
        }
        if let Some(port) = key.strip_prefix("port:") {
            return Some(Radio::Cable {
                port: port.to_owned(),
                model: self.cable_model,
            });
        }
        if let Some(address) = key.strip_prefix("ble:") {
            let name = self
                .bluetooth
                .iter()
                .find(|(a, _)| a == address)
                .map(|(_, n)| n.clone())
                .unwrap_or_default();
            return Some(Radio::Bluetooth {
                address: address.to_owned(),
                name,
                model: self.ble_model,
            });
        }
        None
    }

    fn start(&mut self, job: Job) {
        self.busy = Some(Busy {
            what: job.label(),
            step: String::new(),
            done: 0,
            total: 0,
        });
        self.message = None;
        self.worker.send(job);
    }

    fn select(&mut self, key: String) {
        if self.selected.as_ref() != Some(&key) {
            self.facts.clear();
            self.clock = None;
            self.message = None;
        }
        self.selected = Some(key);
    }

    fn inspect(&mut self, path: PathBuf) {
        self.folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let view = worker::describe_file(&path);
        self.file = Some((path, view));
    }

    fn drain(&mut self) {
        while let Ok(event) = self.worker.rx.try_recv() {
            match event {
                Event::Progress { done, total, what } => {
                    if let Some(b) = &mut self.busy {
                        b.done = done;
                        b.total = total;
                        b.step = what;
                    }
                    continue;
                }
                Event::Found(radios) => {
                    self.radios = radios;
                    if self.selected.is_none() {
                        self.selected = self.radios.first().map(Radio::key);
                    }
                }
                Event::FoundBluetooth(found) => {
                    self.bluetooth = found;
                    if self.bluetooth.is_empty() {
                        self.message =
                            Some(("nothing on Bluetooth advertised a name".into(), false));
                    }
                }
                Event::Identified(facts) => self.facts = facts,
                Event::Saved(path, image) => {
                    self.message = Some((
                        format!("saved {} bytes to {}", image.len(), path.display()),
                        false,
                    ));
                    self.inspect(path);
                }
                Event::Clock(text) => self.clock = Some(text),
                Event::Done(text) => self.message = Some((text, false)),
                Event::Failed(text) => self.message = Some((text, true)),
            }
            self.busy = None;
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("RADIO TOOL")
                    .font(theme::legend_font(15.0))
                    .extra_letter_spacing(3.0)
                    .color(theme::VALUE),
            );
            ui.label(legend(format!("v{}", env!("CARGO_PKG_VERSION"))));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(Key::new("open file")).clicked() {
                    self.browser = Some((
                        Pick::OpenFile,
                        Browser::new("open a firmware or codeplug file", &self.folder),
                    ));
                }
                if let Some(b) = &self.busy {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(legend(b.what));
                }
            });
        });
    }

    fn radio_list(&mut self, ui: &mut Ui) {
        let busy = self.busy.is_some();
        let mut pick = None;
        card(
            ui,
            None,
            |ui| {
                ui.label(legend("usb"));
            },
            |ui| {
                if self.radios.is_empty() {
                    hint(
                        ui,
                        "No radio in bootloader or DFU mode. On a TYT, hold PTT and the button \
                         above it while switching on.",
                    );
                }
                for r in &self.radios {
                    let key = r.key();
                    if row(
                        ui,
                        &r.title(),
                        &r.kind(),
                        &r.place(),
                        self.selected.as_ref() == Some(&key),
                    )
                    .clicked()
                    {
                        pick = Some(key);
                    }
                }
                ui.add_space(4.0);
                if ui.add(Key::new("scan").enabled(!busy)).clicked() {
                    self.ports = worker::serial_ports();
                    self.start(Job::Scan);
                }
            },
        );

        card(
            ui,
            None,
            |ui| {
                ui.label(legend("cable"));
            },
            |ui| {
                if self.ports.is_empty() {
                    hint(ui, "No serial ports. Plug in the programming cable.");
                } else {
                    let ports = self.ports.clone();
                    choice(
                        ui,
                        "port",
                        &mut self.cable_port,
                        ports.into_iter().map(|p| (p.clone(), p)),
                    );
                    let mut model = self.cable_model;
                    choice(
                        ui,
                        "cable model",
                        &mut model,
                        cable_models().into_iter().map(|m| (m, m.to_owned())),
                    );
                    self.cable_model = model;
                    let key = format!("port:{}", self.cable_port);
                    let on = self.selected.as_ref() == Some(&key);
                    if ui
                        .add(Key::new(if on { "selected" } else { "use this cable" }).enabled(!on))
                        .clicked()
                    {
                        pick = Some(key);
                    }
                }
            },
        );

        card(
            ui,
            None,
            |ui| {
                ui.label(legend("bluetooth"));
            },
            |ui| {
                let mut model = self.ble_model;
                choice(
                    ui,
                    "ble model",
                    &mut model,
                    ble_models().into_iter().map(|m| (m, m.to_owned())),
                );
                self.ble_model = model;
                for (address, name) in &self.bluetooth {
                    let key = format!("ble:{address}");
                    if row(
                        ui,
                        name,
                        "bluetooth",
                        address,
                        self.selected.as_ref() == Some(&key),
                    )
                    .clicked()
                    {
                        pick = Some(key);
                    }
                }
                ui.add_space(4.0);
                if ui.add(Key::new("scan bluetooth").enabled(!busy)).clicked() {
                    self.start(Job::ScanBluetooth);
                }
            },
        );

        if let Some(key) = pick {
            self.select(key);
        }
    }

    fn radio_panel(&mut self, ui: &mut Ui) {
        let Some(radio) = self.radio() else {
            if self.file.is_some() {
                return;
            }
            ui.add_space(24.0);
            ui.label(legend("no radio selected"));
            note(
                ui,
                "Pick a radio on the left. A USB radio shows up when it is in bootloader or DFU \
                 mode. A Baofeng on a programming cable or Bluetooth needs its model chosen, \
                 because nothing on the wire says what it is. Or open a file to see what it holds.",
                theme::LEGEND,
            );
            return;
        };
        let busy = self.busy.is_some();

        card(
            ui,
            Some(theme::TRACE),
            |ui| {
                ui.label(
                    RichText::new(radio.title())
                        .size(12.5)
                        .strong()
                        .color(theme::VALUE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    chip(ui, &radio.kind(), theme::LEGEND, false);
                });
            },
            |ui| {
                reading(ui, "where", radio.place());
                for (k, v) in &self.facts {
                    reading(ui, k, v);
                }
                ui.add_space(4.0);
                if ui.add(Key::new("identify").enabled(!busy)).clicked() {
                    self.start(Job::Identify(radio.clone()));
                }
            },
        );

        if radio.reads_codeplug() {
            card(
                ui,
                None,
                |ui| {
                    ui.label(legend("codeplug"));
                },
                |ui| {
                    note(
                        ui,
                        "The channel memory. Read it to keep a copy before writing anything.",
                        theme::LEGEND,
                    );
                    ui.horizontal(|ui| {
                        if ui
                            .add(Key::new("read to file").solid(theme::VALUE).enabled(!busy))
                            .clicked()
                        {
                            self.browser = Some((
                                Pick::SaveCodeplug,
                                Browser::new("save the codeplug as", &self.folder),
                            ));
                        }
                        if ui
                            .add(
                                Key::new("write from file")
                                    .outline(theme::READOUT)
                                    .enabled(!busy),
                            )
                            .clicked()
                        {
                            self.browser = Some((
                                Pick::WriteCodeplug,
                                Browser::new("codeplug to write", &self.folder),
                            ));
                        }
                    });
                },
            );
        }

        if radio.takes_firmware() {
            card(
                ui,
                None,
                |ui| {
                    ui.label(legend("firmware"));
                },
                |ui| {
                    note(
                        ui,
                        "Flashing the wrong file can leave the radio unable to start. The file \
                         is checked against the radio before anything is written.",
                        theme::LEGEND,
                    );
                    if ui
                        .add(
                            Key::new("flash a file")
                                .outline(theme::FAULT)
                                .enabled(!busy),
                        )
                        .clicked()
                    {
                        self.browser =
                            Some((Pick::Flash, Browser::new("firmware to flash", &self.folder)));
                    }
                },
            );
        }

        if radio.has_clock() {
            card(
                ui,
                None,
                |ui| {
                    ui.label(legend("clock"));
                },
                |ui| {
                    if let Some(c) = &self.clock {
                        Line::new().legend("radio").measured(c.clone()).show(ui);
                    }
                    note(
                        ui,
                        "These are commands of the radio's own firmware; a bootloader ignores them.",
                        theme::LEGEND,
                    );
                    ui.horizontal(|ui| {
                        if ui.add(Key::new("read").enabled(!busy)).clicked() {
                            self.start(Job::ReadClock(radio.clone()));
                        }
                        if ui.add(Key::new("set to now").enabled(!busy)).clicked() {
                            self.confirm = Some((Ask::SetClock, None));
                        }
                        if ui.add(Key::new("restart radio").enabled(!busy)).clicked() {
                            self.confirm = Some((Ask::Reboot, None));
                        }
                    });
                },
            );
        }

        if let Some(b) = &self.busy {
            card(
                ui,
                Some(theme::READOUT),
                |ui| {
                    ui.label(legend(b.what));
                },
                |ui| {
                    let total = (b.total > 0).then_some(b.total as f32);
                    let said = if b.total > 0 {
                        format!(
                            "{} of {}",
                            crate::format::format_bytes(b.done as u64),
                            crate::format::format_bytes(b.total as u64)
                        )
                    } else {
                        String::new()
                    };
                    progress(ui, &b.step, b.done as f32, total, &said);
                    hint(ui, "Do not unplug the radio.");
                },
            );
        }
    }

    fn message(&self, ui: &mut Ui) {
        if let Some((text, fault)) = &self.message {
            note(
                ui,
                text.clone(),
                if *fault { theme::FAULT } else { theme::OK },
            );
        }
    }

    fn file_panel(&mut self, ui: &mut Ui) {
        let Some((path, view)) = &self.file else {
            return;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut close = false;
        card(
            ui,
            None,
            |ui| {
                ui.label(legend("file"));
                ui.label(RichText::new(&name).size(12.0).color(theme::VALUE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(Key::new("close")).clicked() {
                        close = true;
                    }
                });
            },
            |ui| match view {
                Ok(FileView::Firmware {
                    kind,
                    facts,
                    segments,
                }) => {
                    Line::new().legend("kind").value(kind.clone()).show(ui);
                    for (k, v) in facts {
                        reading(ui, k, v);
                    }
                    for (ix, (address, len)) in segments.iter().enumerate() {
                        reading(
                            ui,
                            &format!("segment {ix}"),
                            format!(
                                "{address:#010x}, {}",
                                crate::format::format_bytes(*len as u64)
                            ),
                        );
                    }
                }
                Ok(FileView::Codeplug {
                    kind,
                    facts,
                    channels,
                    slots,
                }) => {
                    Line::new().legend("kind").value(kind.clone()).show(ui);
                    for (k, v) in facts {
                        reading(ui, k, v);
                    }
                    if *slots > 0 {
                        reading(
                            ui,
                            "channels",
                            format!("{} of {slots} used", channels.len()),
                        );
                    }
                    if !channels.is_empty() {
                        ui.add_space(6.0);
                        channel_table(ui, channels);
                    }
                }
                Err(e) => note(ui, e.clone(), theme::FAULT),
            },
        );
        if close {
            self.file = None;
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some((_, browser)) = &mut self.browser {
            let picked = browser.show(ctx);
            let chosen = match picked {
                Picked::Browsing => return,
                Picked::Cancelled => None,
                Picked::File(p) => Some(p),
                Picked::Folder(dir) => Some(dir.join("codeplug.img")),
            };
            let Some((pick, browser)) = self.browser.take() else {
                return;
            };
            self.folder = browser.dir.clone();
            let Some(path) = chosen else { return };
            match pick {
                Pick::OpenFile => self.inspect(path),
                Pick::SaveCodeplug => {
                    if let Some(r) = self.radio() {
                        self.start(Job::ReadCodeplug(r, path));
                    }
                }
                Pick::WriteCodeplug => self.confirm = Some((Ask::WriteCodeplug, Some(path))),
                Pick::Flash => self.confirm = Some((Ask::Flash, Some(path))),
            }
            return;
        }

        let Some((ask, path)) = self.confirm.clone() else {
            return;
        };
        let Some(radio) = self.radio() else {
            self.confirm = None;
            return;
        };
        let file = path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let (title, text, key, tint) = match ask {
            Ask::WriteCodeplug => (
                "write the codeplug",
                format!(
                    "Replace the channel memory of {} with {file}. What is on the radio now is lost.",
                    radio.title()
                ),
                "write",
                theme::READOUT,
            ),
            Ask::Flash => (
                "flash firmware",
                format!(
                    "Write {file} to {}. Do not unplug the radio until it says it is done.",
                    radio.title()
                ),
                "flash",
                theme::FAULT,
            ),
            Ask::SetClock => (
                "set the clock",
                format!("Set {} to this computer's local time.", radio.title()),
                "set",
                theme::READOUT,
            ),
            Ask::Reboot => (
                "restart the radio",
                format!("Ask {} to restart.", radio.title()),
                "restart",
                theme::READOUT,
            ),
        };
        let mut go = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
            ui.set_width(440.0);
            egui_bench::prelude::modal_title(ui, title);
            note(ui, text, theme::VALUE);
            if let Some(p) = &path
                && let Ok(view) = worker::describe_file(p)
            {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(view.summary())
                        .font(theme::mono(11.0))
                        .color(theme::LEGEND),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add(Key::new(key).solid(tint)).clicked() {
                    go = true;
                }
                if ui.add(Key::new("cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if cancel || modal.should_close() {
            self.confirm = None;
        } else if go {
            self.confirm = None;
            let path = path.unwrap_or_default();
            self.start(match ask {
                Ask::WriteCodeplug => Job::WriteCodeplug(radio, path),
                Ask::Flash => Job::Flash(radio, path),
                Ask::SetClock => Job::SetClock(radio),
                Ask::Reboot => Job::Reboot(radio),
            });
        }
    }
}

fn channel_table(ui: &mut Ui, channels: &[worker::ChannelRow]) {
    const COLUMNS: &[(&str, f32)] = &[
        ("#", 34.0),
        ("name", 110.0),
        ("rx mhz", 90.0),
        ("tx mhz", 90.0),
        ("rx tone", 70.0),
        ("tx tone", 70.0),
        ("mode", 64.0),
        ("power", 56.0),
        ("scan", 40.0),
    ];
    Table::new(COLUMNS, channels.len()).show(ui, |i, p, r, at| {
        let Some(c) = channels.get(i) else { return };
        let cols = [
            (c.slot.to_string(), theme::LEGEND),
            (c.name.clone(), theme::VALUE),
            (c.rx.clone(), theme::VALUE),
            (c.tx.clone(), theme::VALUE),
            (c.rx_tone.clone(), theme::LEGEND),
            (c.tx_tone.clone(), theme::LEGEND),
            (c.mode.clone(), theme::LEGEND),
            (c.power.clone(), theme::LEGEND),
            (
                (if c.scan { "yes" } else { "no" }).to_owned(),
                theme::LEGEND,
            ),
        ];
        for (n, (text, colour)) in cols.iter().enumerate() {
            let w = COLUMNS.get(n).map_or(60.0, |(_, w)| *w);
            cell(p, r, at(n), w, text, *colour);
        }
    });
}

fn row(ui: &mut Ui, title: &str, kind: &str, place: &str, on: bool) -> egui::Response {
    const H: f32 = 50.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), H), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if on {
            theme::READOUT.gamma_multiply(0.12)
        } else if resp.hovered() {
            theme::BAND
        } else {
            theme::WELL
        };
        let p = ui.painter();
        p.rect_filled(rect, 2.0, fill);
        p.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, if on { theme::READOUT } else { theme::ETCH }),
            egui::StrokeKind::Inside,
        );
        if on {
            p.rect_filled(
                egui::Rect::from_min_size(rect.min, egui::vec2(3.0, H)),
                0.0,
                theme::READOUT,
            );
        }
        let x = rect.left() + 10.0;
        p.text(
            egui::pos2(x, rect.top() + 7.0),
            egui::Align2::LEFT_TOP,
            title,
            theme::figure(12.5),
            theme::VALUE,
        );
        p.text(
            egui::pos2(x, rect.top() + 24.0),
            egui::Align2::LEFT_TOP,
            kind.to_uppercase(),
            theme::legend_font(10.0),
            if on { theme::READOUT } else { theme::LEGEND },
        );
        p.text(
            egui::pos2(x, rect.top() + 37.0),
            egui::Align2::LEFT_TOP,
            place,
            theme::mono(9.5),
            theme::LEGEND,
        );
    }
    resp
}

impl eframe::App for App {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.drain();

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            self.inspect(path);
        }

        egui::Panel::top("head")
            .frame(egui::Frame::NONE.fill(theme::WELL).inner_margin(8))
            .show(root, |ui| self.header(ui));

        egui::Panel::left("radios")
            .exact_size(SIDE_W)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(theme::CHASSIS).inner_margin(8))
            .show(root, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        self.radio_list(ui);
                    });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::CHASSIS).inner_margin(8))
            .show(root, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                        self.message(ui);
                        self.radio_panel(ui);
                        self.file_panel(ui);
                    });
            });

        if egui_bench::drop::hovering(&ctx).is_some() {
            egui_bench::drop::overlay(
                &ctx,
                None,
                "drop to inspect",
                "a firmware or codeplug file",
                theme::READOUT,
            );
        }

        self.dialogs(&ctx);
    }
}

pub fn run(open: Option<PathBuf>) -> std::process::ExitCode {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1000.0, 720.0])
        .with_min_inner_size([760.0, 520.0])
        .with_title("radio_tool")
        .with_app_id("radio_tool")
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icon-256.png"))
    {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let result = eframe::run_native(
        "radio_tool",
        options,
        Box::new(move |cc| {
            egui_bench::install(&cc.egui_ctx);
            App::new(&cc.egui_ctx, open)
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(Into::into)
        }),
    );
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: cannot open the window: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    struct Simulator(Child, String);

    impl Drop for Simulator {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn simulator(dir: &Path) -> Option<Simulator> {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../test/fake_uv5r.py");
        let log = dir.join("port.txt");
        let child = Command::new("python3")
            .arg("-u")
            .arg(&script)
            .stdout(Stdio::from(std::fs::File::create(&log).ok()?))
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(port) = std::fs::read_to_string(&log).ok()?.lines().next() {
                return Some(Simulator(child, port.trim().to_owned()));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        None
    }

    fn settle(harness: &mut egui_kittest::Harness<'_, App>) {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            harness.step();
            if harness.state().busy.is_none() || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    #[ignore = "writes screenshots, run by hand"]
    fn screenshots() {
        let out = std::env::temp_dir().join("radio_tool_shots");
        std::fs::create_dir_all(&out).unwrap();
        let dir = std::env::temp_dir().join(format!("radio_tool_shots_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sim = simulator(&dir).unwrap();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1000.0, 720.0))
            .wgpu()
            .build_eframe(|cc| {
                egui_bench::install(&cc.egui_ctx);
                App::new(&cc.egui_ctx, None).unwrap()
            });
        settle(&mut harness);
        harness.state_mut().ports.push(sim.1.clone());
        harness.state_mut().cable_port = sim.1.clone();
        harness.state_mut().select(format!("port:{}", sim.1));
        let radio = harness.state().radio().unwrap();
        harness.state_mut().start(Job::Identify(radio));
        settle(&mut harness);
        harness.run_steps(3);
        harness
            .render()
            .unwrap()
            .save(out.join("radio.png"))
            .unwrap();

        let image = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../codeplug/tests/fixtures/uv5r_hn5rv01.img");
        harness.state_mut().confirm = Some((Ask::WriteCodeplug, Some(image)));
        harness.run_steps(3);
        harness
            .render()
            .unwrap()
            .save(out.join("confirm.png"))
            .unwrap();

        harness.state_mut().confirm = None;
        harness.state_mut().busy = Some(Busy {
            what: "writing the codeplug",
            step: "write".into(),
            done: 3000,
            total: 6400,
        });
        harness.run_steps(3);
        harness
            .render()
            .unwrap()
            .save(out.join("busy.png"))
            .unwrap();
        drop(sim);
    }

    #[test]
    fn the_window_identifies_and_reads_a_radio_on_a_cable() {
        let dir = std::env::temp_dir().join(format!("radio_tool_gui_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let Some(sim) = simulator(&dir) else {
            eprintln!("skipping: the UV-5R simulator did not start");
            return;
        };

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1000.0, 720.0))
            .build_eframe(|cc| {
                egui_bench::install(&cc.egui_ctx);
                App::new(&cc.egui_ctx, None).unwrap()
            });
        settle(&mut harness);

        let port = sim.1.clone();
        harness.state_mut().cable_port = port.clone();
        harness.state_mut().cable_model = "UV5R";
        harness.state_mut().select(format!("port:{port}"));

        let radio = harness.state().radio().unwrap();
        harness.state_mut().start(Job::Identify(radio));
        settle(&mut harness);
        let facts = harness.state().facts.clone();
        assert!(
            facts.iter().any(|(k, v)| k == "firmware" && v == "HN5RV01"),
            "{facts:?} {:?}",
            harness.state().message
        );
        drop(sim);

        let sim = simulator(&dir).unwrap();
        harness.state_mut().cable_port = sim.1.clone();
        harness.state_mut().select(format!("port:{}", sim.1));
        let image = dir.join("read.img");
        let radio = harness.state().radio().unwrap();
        harness
            .state_mut()
            .start(Job::ReadCodeplug(radio, image.clone()));
        settle(&mut harness);

        let reference = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../codeplug/tests/fixtures/uv5r_hn5rv01.img"),
        )
        .unwrap();
        assert!(
            std::fs::read(&image).unwrap() == reference,
            "{:?}",
            harness.state().message
        );
        let shown = harness
            .state()
            .file
            .as_ref()
            .map(|(p, v)| (p.clone(), v.is_ok()));
        assert_eq!(shown, Some((image, true)));

        drop(sim);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
