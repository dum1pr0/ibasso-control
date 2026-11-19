#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke};
use hidapi::HidApi;
use ibasso_control::hid::{self, Command, Handle, Snapshot, UsbEntry};
use ibasso_control::ibasso::{Control, Ctl, Family, PeqBand, MODELS};
use ibasso_control::settings::Settings;
use ibasso_control::{icon, udev};
use ibasso_control::tray::{self, Signal};
use ibasso_control::{autostart, instance};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FILTER_TYPES: [&str; 8] =
    ["Tắt", "Low shelf", "Peak", "High shelf", "Low pass", "High pass", "Band pass", "All pass"];

/// Stone neutrals with a gold accent ("premium black + gold"), checked for 4.5:1 text contrast in both modes.
#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    panel: Color32,
    card: Color32,
    border: Color32,
    muted: Color32,
    accent: Color32,
    on_accent: Color32,
    good: Color32,
    warn: Color32,
    danger: Color32,
    message_bg: Color32,
    message_text: Color32,
}

const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x0c, 0x0a, 0x09),
    panel: Color32::from_rgb(0x14, 0x12, 0x10),
    card: Color32::from_rgb(0x1c, 0x19, 0x17),
    border: Color32::from_rgb(0x33, 0x30, 0x2d),
    muted: Color32::from_rgb(0xa8, 0xa2, 0x9e),
    accent: Color32::from_rgb(0xca, 0x8a, 0x04),
    on_accent: Color32::from_rgb(0x0c, 0x0a, 0x09),
    good: Color32::from_rgb(0x4a, 0xde, 0x80),
    warn: Color32::from_rgb(0xfb, 0x92, 0x3c),
    danger: Color32::from_rgb(0xf8, 0x71, 0x71),
    message_bg: Color32::from_rgb(0x3b, 0x2a, 0x0a),
    message_text: Color32::from_rgb(0xfd, 0xe6, 0x8a),
};

const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xfa, 0xfa, 0xf9),
    panel: Color32::from_rgb(0xf5, 0xf5, 0xf4),
    card: Color32::from_rgb(0xff, 0xff, 0xff),
    border: Color32::from_rgb(0xd6, 0xd3, 0xd1),
    muted: Color32::from_rgb(0x57, 0x53, 0x4e),
    accent: Color32::from_rgb(0xa1, 0x62, 0x07),
    on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
    good: Color32::from_rgb(0x15, 0x80, 0x3d),
    warn: Color32::from_rgb(0xc2, 0x41, 0x0c),
    danger: Color32::from_rgb(0xdc, 0x26, 0x26),
    message_bg: Color32::from_rgb(0xfe, 0xf3, 0xc7),
    message_text: Color32::from_rgb(0x71, 0x3f, 0x12),
};

fn pal(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

/// Space between cards, both between rows and between the two columns.
const GAP: f32 = 16.0;

/// Control groups shown as separate cards, in display order.
const SECTIONS: [(&str, &[Ctl]); 3] = [
    (
        "Âm thanh",
        &[
            Ctl::Volume,
            Ctl::Balance,
            Ctl::Gain,
            Ctl::DacGain,
            Ctl::OpVoltage,
            Ctl::AmpMode,
            Ctl::WorkMode,
            Ctl::Output,
            Ctl::Filter,
            Ctl::HpFilter,
            Ctl::Eq,
            Ctl::Dre,
            Ctl::PcmTrim,
        ],
    ),
    ("Kết nối và phím bấm", &[Ctl::Spdif, Ctl::MicMode, Ctl::KeyControl]),
    ("Màn hình và hệ thống", &[Ctl::ScreenRotation, Ctl::ScreenTime, Ctl::Language]),
];

struct Connection {
    entry: UsbEntry,
    family: Family,
    handle: Handle,
    state: Snapshot,
    /// Saved values re-sent on connect to a model that cannot report its settings.
    restored: Vec<(Ctl, i32)>,
    peq_slot: u8,
    peq_total_gain: i32,
    peq_edit: Option<[[PeqBand; 10]; 3]>,
    peq_dirty: bool,
}

struct App {
    api: Option<HidApi>,
    usb: Vec<UsbEntry>,
    last_scan: Instant,
    connection: Option<Connection>,
    message: Option<String>,
    saved: Settings,
    /// False when the desktop has no system tray; closing the window then quits.
    tray: bool,
    autostart: bool,
    /// The settings page replaces the device controls in the main area while open.
    settings_open: bool,
}

impl App {
    fn new(tray: bool) -> Self {
        let (api, message) = match HidApi::new() {
            Ok(api) => (Some(api), None),
            Err(e) => (None, Some(format!("Không khởi tạo được USB HID: {e}"))),
        };
        let autostart = autostart::is_enabled();
        if autostart {
            // Keeps the login entry pointing at this copy of the app if it was moved.
            let _ = autostart::set(true);
        }
        let mut app = App {
            api,
            usb: Vec::new(),
            last_scan: Instant::now(),
            connection: None,
            message,
            saved: Settings::load(),
            tray,
            autostart,
            settings_open: false,
        };
        app.rescan();
        app
    }

    /// Bottom of the sidebar: a gear button, on the left, that opens the settings page.
    fn footer(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let open = self.settings_open;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
        let response = response.on_hover_text("Cài đặt");
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, "Cài đặt"));
        let (color, fill, stroke) = if open {
            (p.accent, p.accent.gamma_multiply(0.14), p.accent)
        } else if response.hovered() {
            (ui.visuals().strong_text_color(), ui.visuals().widgets.hovered.weak_bg_fill, p.border)
        } else {
            (p.muted, Color32::TRANSPARENT, p.border)
        };
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::same(10), fill, Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
        gear(painter, rect.center(), 11.0, color);
        if response.clicked() {
            self.settings_open = !open;
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("TÙY CHỌN").small().strong().color(p.muted));
                ui.label(RichText::new("Cài đặt").size(28.0).strong());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.button("Đóng").clicked() {
                    self.settings_open = false;
                }
            });
        });
        gap(ui);

        card(ui, |ui| {
            section_title(ui, "Giao diện");
            ui.label(RichText::new("Chế độ màu").strong());
            theme_switcher(ui);
        });
        gap(ui);

        card(ui, |ui| {
            section_title(ui, "Khởi động và chạy ngầm");
            let mut enabled = self.autostart;
            let changed = ui
                .horizontal(|ui| {
                    let switch = toggle_switch(ui, &mut enabled);
                    let label = ui.add(egui::Label::new(RichText::new("Chạy khi khởi động máy").strong()).sense(egui::Sense::click()));
                    if label.clicked() {
                        enabled = !enabled;
                    }
                    switch.changed() || label.clicked()
                })
                .inner;
            if changed {
                match autostart::set(enabled) {
                    Ok(()) => self.autostart = enabled,
                    Err(e) => self.message = Some(e),
                }
            }
            let hint = if self.tray {
                "Khi đăng nhập, app tự chạy và chỉ hiện icon dưới khay hệ thống."
            } else {
                "Khi đăng nhập, app tự mở."
            };
            ui.label(RichText::new(hint).color(p.muted));
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new("Khi đóng cửa sổ").strong());
            let text = if self.tray {
                "Nút X chỉ ẩn cửa sổ, app vẫn chạy và giữ kết nối với DAC. Bấm icon dưới khay hoặc mở lại app để hiện cửa sổ. Thoát hẳn bằng mục Thoát trong menu của icon khay hoặc của nút app trên taskbar."
            } else {
                "Không tìm thấy khay hệ thống, nên nút X sẽ thoát app."
            };
            ui.label(RichText::new(text).color(p.muted));
        });
        gap(ui);

        card(ui, |ui| {
            section_title(ui, "Thông tin");
            ui.label(format!("iBasso Control {}", env!("CARGO_PKG_VERSION")));
            ui.hyperlink_to("Mã nguồn trên GitHub", env!("CARGO_PKG_REPOSITORY"));
            ui.label(RichText::new("Dự án không chính thức, không liên quan tới iBasso.").small().color(p.muted));
        });
    }

    fn rescan(&mut self) {
        self.last_scan = Instant::now();
        if let Some(api) = self.api.as_mut() {
            self.usb = hid::scan(api);
        }
    }

    fn connect(&mut self, entry: UsbEntry, family: Family) {
        self.disconnect();
        let Some(api) = self.api.as_ref() else { return };
        match hid::open(api, &entry, family) {
            Ok(handle) => {
                self.message = None;
                let mut state = Snapshot::default();
                // Models that cannot report their settings get the last values back, as the iBasso app does.
                // Volume comes first because the balance command depends on it.
                let restored = if family.reports_state() { Vec::new() } else { self.saved.for_device(&entry.product) };
                for (ctl, value) in &restored {
                    state.values.insert(*ctl, *value);
                    let _ = handle.tx.send(Command::Set(*ctl, *value));
                }
                self.connection = Some(Connection {
                    entry,
                    family,
                    handle,
                    state,
                    restored,
                    peq_slot: 0,
                    peq_total_gain: 0,
                    peq_edit: None,
                    peq_dirty: false,
                });
            }
            Err(e) => {
                let hint = if cfg!(target_os = "linux") { " Hãy bấm \"Cài quyền udev\" rồi cắm lại thiết bị." } else { "" };
                self.message = Some(format!("{e}.{hint}"));
            }
        }
    }

    fn disconnect(&mut self) {
        if let Some(connection) = self.connection.take() {
            let _ = connection.handle.tx.send(Command::Close);
        }
    }

    fn pump(&mut self) {
        let mut lost = false;
        if let Some(connection) = self.connection.as_mut() {
            while let Ok(snapshot) = connection.handle.rx.try_recv() {
                if let Some(e) = &snapshot.error {
                    self.message = Some(e.clone());
                }
                lost |= snapshot.closed;
                if snapshot.progress.is_some() {
                    connection.state.progress = snapshot.progress;
                    continue;
                }
                if !snapshot.closed {
                    if !connection.peq_dirty {
                        connection.peq_edit = snapshot.peq;
                        connection.peq_total_gain = snapshot.peq_gain[connection.peq_slot as usize] as i32;
                    }
                    connection.state = snapshot;
                }
            }
        }
        if lost {
            self.connection = None;
        }
        if self.last_scan.elapsed() > Duration::from_secs(2) {
            self.rescan();
        }
    }

    fn device_list(&mut self, ui: &mut egui::Ui) {
    let p = pal(ui);
        ui.horizontal(|ui| {
            logo(ui, 36.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("iBasso Control").size(20.0).strong());
                ui.label(RichText::new("DAC/amp USB").small().color(p.muted));
            });
        });
        ui.add_space(14.0);
        ui.label(RichText::new("THIẾT BỊ").small().strong().color(p.muted));
        ui.add_space(4.0);
        if self.usb.is_empty() {
            card(ui, |ui| {
                ui.label(RichText::new("Chưa thấy thiết bị nào.").color(p.muted));
                ui.label(RichText::new("Hãy cắm DAC vào cổng USB.").small().color(p.muted));
            });
        }
        let current = self.connection.as_ref().map(|c| c.entry.path.clone());
        let mut connect = None;
        for entry in &self.usb {
            let selected = current.as_ref() == Some(&entry.path);
            let stroke = if selected { Stroke::new(1.5_f32, p.accent) } else { Stroke::new(1.0_f32, p.border) };
            egui::Frame::new()
                .fill(p.card)
                .stroke(stroke)
                .corner_radius(10)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let dot = if selected { p.good } else if entry.family.is_some() { p.muted } else { p.warn };
                        ui.label(RichText::new("●").color(dot));
                        ui.label(
                            RichText::new(if entry.product.is_empty() { "(không tên)" } else { &entry.product }).strong(),
                        );
                    });
                    ui.label(RichText::new(format!("{:04x}:{:04x}", entry.vid, entry.pid)).small().color(p.muted));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        match entry.family {
                            Some(_) if selected => {
                                ui.label(RichText::new("Đang kết nối").small().strong().color(p.good));
                            }
                            Some(family) => {
                                if accent_button(ui, "Kết nối").clicked() {
                                    connect = Some((entry.clone(), family));
                                }
                            }
                            None => {
                                ui.label(RichText::new("Model chưa hỗ trợ").small().color(p.warn));
                            }
                        }
                        if cfg!(target_os = "linux") {
                            let installed = udev::is_installed(entry.vid, entry.pid);
                            let label = if installed { "Cài lại quyền udev" } else { "Cài quyền udev" };
                            if ui.button(label).on_hover_text(udev::rule_path(entry.vid, entry.pid)).clicked() {
                                self.message = Some(match udev::install(entry.vid, entry.pid) {
                                    Ok(()) => "Đã cài quyền udev. Hãy rút và cắm lại thiết bị.".into(),
                                    Err(e) => e,
                                });
                            }
                        }
                    });
                });
            ui.add_space(GAP / 2.0 - ui.spacing().item_spacing.y);
        }
        if let Some((entry, family)) = connect {
            self.settings_open = false;
            self.connect(entry, family);
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let Some(connection) = self.connection.as_mut() else {
            welcome(ui);
            return;
        };
        let reads_state = connection.family.reports_state();
        let ready = connection.state.ready;
        let mut close = false;

        // Page header: what is being controlled, its status, and the separated destructive action.
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("ĐANG ĐIỀU KHIỂN").small().strong().color(p.muted));
                ui.label(RichText::new(&connection.entry.product).size(28.0).strong());
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if let Some(firmware) = &connection.state.firmware {
                        chip(ui, &format!("Firmware {firmware}"), p.accent);
                    }
                    chip(ui, &format!("{:04x}:{:04x}", connection.entry.vid, connection.entry.pid), p.muted);
                    if !reads_state {
                        chip(ui, "Không báo trạng thái", p.warn);
                    } else if ready {
                        chip(ui, "Đã đồng bộ", p.good);
                    }
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let button = egui::Button::new(RichText::new("Ngắt kết nối").color(p.danger))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0_f32, p.danger.gamma_multiply(0.6)));
                close = ui.add(button).clicked();
            });
        });
        gap(ui);

        if reads_state && !ready {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(18.0).color(p.accent));
                    ui.label(RichText::new("Đang đọc cài đặt từ thiết bị…").strong());
                });
                ui.add_space(8.0);
                let (done, total) = connection.state.progress.unwrap_or((0, 0));
                let fraction = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                let text = if total == 0 { "Đang mở kết nối".to_string() } else { format!("{done}/{total} nhóm") };
                ui.add(egui::ProgressBar::new(fraction).text(text).fill(p.accent).animate(true));
            });
            gap(ui);
        }

        if !reads_state {
            card(ui, |ui| {
                ui.label(
                    RichText::new("Máy này không cho đọc cài đặt hiện tại. App ghi nhớ lần chỉnh gần nhất và tự gửi lại khi kết nối, như app iBasso.")
                        .color(p.muted),
                );
                if !connection.restored.is_empty() {
                    let controls = connection.family.controls();
                    let text: Vec<String> = connection
                        .restored
                        .iter()
                        .filter_map(|(ctl, value)| {
                            let control = controls.iter().find(|c| c.id == *ctl)?;
                            Some(format!("{} {}", ctl.label(), shown_value(control, *value)))
                        })
                        .collect();
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("Đã gửi lại: {}", text.join(" · "))).small().color(p.good));
                }
            });
            gap(ui);
        }

        let controls = connection.family.controls();
        let find = |id: Ctl| controls.iter().find(|c| c.id == id);
        let mut sent = Vec::new();
        ui.add_enabled_ui(ready || !reads_state, |ui| {
            // The most used control gets the most visual weight.
            if let Some(volume) = find(Ctl::Volume) {
                card(ui, |ui| hero_volume(ui, volume, find(Ctl::Balance), &mut connection.state, &mut sent));
                gap(ui);
            }
            let sections: Vec<(&str, Vec<&Control>)> = SECTIONS
                .iter()
                .map(|(title, members)| {
                    let group = members
                        .iter()
                        .filter(|id| !matches!(id, Ctl::Volume | Ctl::Balance))
                        .filter_map(|id| find(*id))
                        .collect::<Vec<_>>();
                    (*title, group)
                })
                .filter(|(_, group)| !group.is_empty())
                .collect();
            if ui.available_width() >= 640.0 && sections.len() > 1 {
                // Sound settings on the left, device and screen settings stacked on the right.
                ui.spacing_mut().item_spacing.x = GAP;
                ui.columns(2, |columns| {
                    for (index, (title, group)) in sections.iter().enumerate() {
                        let column = &mut columns[index.min(1)];
                        if index > 1 {
                            gap(column);
                        }
                        settings_card(column, title, group, &mut connection.state, &mut sent);
                    }
                });
                gap(ui);
            } else {
                for (title, group) in &sections {
                    settings_card(ui, title, group, &mut connection.state, &mut sent);
                    gap(ui);
                }
            }
            if connection.family.has_peq() {
                card(ui, |ui| {
                    egui::CollapsingHeader::new(RichText::new("EQ tham số").size(17.0).strong())
                        .default_open(false)
                        .show(ui, |ui| peq_editor(ui, connection));
                });
            }
        });
        let product = connection.entry.product.clone();
        for (ctl, value) in sent {
            let _ = connection.handle.tx.send(Command::Set(ctl, value));
            self.saved.remember(&product, ctl, value);
        }
        if close {
            self.disconnect();
        }
    }
}

/// Volume as the primary control: a large readout, a full-width slider, and balance beneath it.
fn hero_volume(
    ui: &mut egui::Ui,
    volume: &Control,
    balance: Option<&Control>,
    state: &mut Snapshot,
    sent: &mut Vec<(Ctl, i32)>,
) {
    let p = pal(ui);
    let known = state.values.get(&Ctl::Volume).copied();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Âm lượng").size(17.0).strong());
        if known.is_none() {
            chip(ui, "Chưa rõ", p.warn);
        }
    });
    ui.horizontal(|ui| {
        let shown = known.map_or_else(|| "–".to_string(), |v| volume.describe(v));
        ui.label(RichText::new(shown).size(48.0).strong().color(p.accent));
        ui.label(RichText::new(format!("/ {}", volume.describe(volume.max))).size(18.0).color(p.muted));
    });
    if let Some(value) = slider_row(ui, volume, state, false) {
        sent.push((Ctl::Volume, value));
    }
    if let Some(balance) = balance {
        ui.add_space(12.0);
        field_label(ui, balance, state);
        if let Some(value) = slider_row(ui, balance, state, true) {
            sent.push((Ctl::Balance, value));
        }
    }
}

/// A titled card of settings, one per row: name above, control below, rows divided by a hairline.
fn settings_card(ui: &mut egui::Ui, title: &str, group: &[&Control], state: &mut Snapshot, sent: &mut Vec<(Ctl, i32)>) {
    card(ui, |ui| {
        section_title(ui, title);
        for (index, control) in group.iter().enumerate() {
            if index > 0 {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
            }
            field_label(ui, control, state);
            if let Some(value) = control_widget(ui, control, state) {
                sent.push((control.id, value));
            }
        }
    });
}

fn field_label(ui: &mut egui::Ui, control: &Control, state: &Snapshot) {
    let p = pal(ui);
    ui.horizontal(|ui| {
        ui.label(RichText::new(control.id.label()).strong());
        if !state.values.contains_key(&control.id) {
            ui.label(RichText::new("Chưa rõ").small().color(p.warn));
        }
    });
}

/// A slider that fills the row. Returns the value to send once the user lets go.
fn slider_row(ui: &mut egui::Ui, control: &Control, state: &mut Snapshot, show_value: bool) -> Option<i32> {
    let known = state.values.get(&control.id).copied();
    // An unknown volume starts at the quiet end; an unknown balance starts in the middle.
    let fallback = if control.id == Ctl::Balance { (control.min + control.max) / 2 } else { control.min };
    let mut value = known.unwrap_or(fallback);
    let response = ui
        .horizontal(|ui| {
            let reserve = if show_value { 76.0 } else { 0.0 };
            ui.spacing_mut().slider_width = (ui.available_width() - reserve).max(120.0);
            let slider = egui::Slider::new(&mut value, control.min..=control.max)
                .show_value(show_value)
                .custom_formatter(|v, _| control.describe(v as i32));
            ui.add(slider)
        })
        .inner;
    if response.changed() {
        state.values.insert(control.id, value);
    }
    (response.drag_stopped() || (response.changed() && !response.dragged())).then_some(value)
}

/// Draws one control and returns the value to send when the user changed it.
fn control_widget(ui: &mut egui::Ui, control: &Control, state: &mut Snapshot) -> Option<i32> {
    if control.options.is_empty() {
        return slider_row(ui, control, state, true);
    }
    let known = state.values.get(&control.id).copied();
    let mut value = known.unwrap_or(i32::MIN);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let padding = ui.spacing().button_padding.x * 2.0 + 4.0;
    let needed: f32 = 8.0
        + control
            .options
            .iter()
            .map(|(_, label)| ui.fonts(|f| f.layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE).size().x) + padding)
            .sum::<f32>();
    if control.options.len() <= 4 && needed <= ui.available_width() {
        // Segmented control: the choices sit together in one recessed track.
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .corner_radius(10)
            .inner_margin(Margin::same(4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (option, label) in control.options {
                        if ui.selectable_label(value == *option, *label).clicked() {
                            value = *option;
                        }
                    }
                });
            });
    } else {
        let text = known.and_then(|v| control.option_label(v)).unwrap_or("Chưa chọn");
        let width = ui.available_width().min(320.0);
        egui::ComboBox::from_id_salt(control.id.label()).selected_text(text).width(width).show_ui(ui, |ui| {
            for (option, label) in control.options {
                ui.selectable_value(&mut value, *option, *label);
            }
        });
    }
    let changed = (Some(value) != known && value != i32::MIN).then_some(value);
    if let Some(value) = changed {
        state.values.insert(control.id, value);
    }
    changed
}

fn shown_value(control: &Control, value: i32) -> String {
    control.option_label(value).map_or_else(|| control.describe(value), str::to_string)
}

fn welcome(ui: &mut egui::Ui) {
    let p = pal(ui);
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        logo(ui, 56.0);
        ui.label(RichText::new("Chọn một thiết bị để điều khiển").size(22.0).strong());
        ui.label(RichText::new("Cắm DAC iBasso vào máy tính rồi bấm Kết nối ở danh sách bên trái.").color(p.muted));
    });
    ui.add_space(24.0);
    card(ui, |ui| {
        section_title(ui, "Model được hỗ trợ");
        ui.horizontal_wrapped(|ui| {
            for model in MODELS {
                chip(ui, model, p.muted);
            }
        });
    });
}

fn peq_editor(ui: &mut egui::Ui, connection: &mut Connection) {
    let p = pal(ui);
    let Some(edit) = connection.peq_edit.as_mut() else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().color(p.accent));
            ui.label(RichText::new("Đang đọc EQ từ thiết bị…").color(p.muted));
        });
        return;
    };
    let previous_slot = connection.peq_slot;
    let previous_gain = connection.peq_total_gain;
    ui.horizontal(|ui| {
        for slot in 0..3u8 {
            ui.selectable_value(&mut connection.peq_slot, slot, format!("Bộ {}", slot + 1));
        }
        ui.add_space(16.0);
        ui.label(RichText::new("Gain tổng").color(p.muted));
        ui.add(egui::DragValue::new(&mut connection.peq_total_gain).range(-24..=24).suffix(" dB"));
    });
    ui.add_space(6.0);
    let slot = connection.peq_slot as usize;
    if connection.peq_slot != previous_slot {
        connection.peq_total_gain = connection.state.peq_gain[slot] as i32;
    } else if connection.peq_total_gain != previous_gain {
        connection.peq_dirty = true;
    }
    let before = edit[slot];
    egui::Grid::new("peq").num_columns(5).striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
        for title in ["Dải", "Loại", "Tần số (Hz)", "Gain (dB)", "Q"] {
            ui.label(RichText::new(title).small().strong().color(p.muted));
        }
        ui.end_row();
        for (index, band) in edit[slot].iter_mut().enumerate() {
            ui.label(format!("{}", index + 1));
            let current = FILTER_TYPES.get(band.filter_type as usize).copied().unwrap_or("Khác");
            egui::ComboBox::from_id_salt(("peq_type", index)).selected_text(current).show_ui(ui, |ui| {
                for (value, name) in FILTER_TYPES.iter().enumerate() {
                    ui.selectable_value(&mut band.filter_type, value as u8, *name);
                }
            });
            ui.add(egui::DragValue::new(&mut band.frequency).range(20..=20000));
            ui.add(egui::DragValue::new(&mut band.gain).range(-12.0..=12.0).speed(0.1));
            ui.add(egui::DragValue::new(&mut band.q).range(0.1..=10.0).speed(0.01));
            ui.end_row();
        }
    });
    if edit[slot] != before {
        connection.peq_dirty = true;
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if accent_button(ui, "Ghi vào thiết bị").clicked() {
            connection.peq_dirty = false;
            connection.state.peq_gain[slot] = connection.peq_total_gain as i8;
            let _ = connection.handle.tx.send(Command::WritePeq {
                slot: connection.peq_slot,
                bands: Box::new(edit[slot]),
                total_gain: connection.peq_total_gain as i8,
            });
        }
        if connection.peq_dirty {
            ui.label(RichText::new("Có thay đổi chưa ghi").small().color(p.warn));
        }
    });
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(12)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// App mark drawn as vector shapes with the same geometry as the window and .exe icon.
fn logo(ui: &mut egui::Ui, size: f32) {
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same((size * icon::CORNER) as u8), p.accent);
    for (cx, cy, hw, hh) in icon::bar_rects(size) {
        let bar = egui::Rect::from_center_size(rect.min + egui::vec2(cx, cy), egui::vec2(hw * 2.0, hh * 2.0));
        painter.rect_filled(bar, CornerRadius::same(hw as u8), Color32::from_rgb(0x16, 0x12, 0x0e));
    }
}

/// A gear drawn from strokes: a ring with eight teeth around it.
fn gear(painter: &egui::Painter, centre: egui::Pos2, radius: f32, color: Color32) {
    let ring = radius * 0.56;
    painter.circle_stroke(centre, ring, Stroke::new(radius * 0.34, color));
    for tooth in 0..8 {
        let angle = tooth as f32 * std::f32::consts::TAU / 8.0;
        let direction = egui::vec2(angle.cos(), angle.sin());
        painter.line_segment([centre + direction * ring, centre + direction * radius], Stroke::new(radius * 0.4, color));
    }
}

/// An on/off switch: a pill track that fills with the accent colour when on.
fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let p = pal(ui);
    let size = egui::vec2(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "Chạy khi khởi động máy"));
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let track = if *on { p.accent } else { ui.visuals().widgets.inactive.bg_fill };
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(11), track, Stroke::new(1.0_f32, p.border), egui::StrokeKind::Inside);
    let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
    let knob = if *on { p.on_accent } else { p.muted };
    painter.circle_filled(egui::pos2(x, rect.center().y), 8.0, knob);
    response
}

/// Adds space so the distance to the next item is exactly GAP.
fn gap(ui: &mut egui::Ui) {
    ui.add_space(GAP - ui.spacing().item_spacing.y);
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).size(17.0).strong());
    ui.add_space(12.0);
}

fn chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.add(
        egui::Button::new(RichText::new(text).small().color(color))
            .fill(color.gamma_multiply(0.16))
            .stroke(Stroke::NONE)
            .corner_radius(8)
            .wrap_mode(egui::TextWrapMode::Extend)
            .sense(egui::Sense::hover()),
    );
}

fn accent_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let p = pal(ui);
    ui.add(egui::Button::new(RichText::new(text).strong().color(p.on_accent)).fill(p.accent))
}

fn apply_theme(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let dark = theme == egui::Theme::Dark;
        let p = if dark { DARK } else { LIGHT };
        let rgb = |dark_value: u32, light_value: u32| {
            let v = if dark { dark_value } else { light_value };
            Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
        };
        ctx.style_mut_of(theme, |style| {
            use egui::{FontId, TextStyle};
            style.text_styles.insert(TextStyle::Body, FontId::proportional(16.0));
            style.text_styles.insert(TextStyle::Button, FontId::proportional(15.0));
            style.text_styles.insert(TextStyle::Small, FontId::proportional(13.0));
            style.text_styles.insert(TextStyle::Heading, FontId::proportional(22.0));
            style.spacing.item_spacing = egui::vec2(10.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 6.0);
            style.spacing.interact_size.y = 32.0;
            style.spacing.slider_rail_height = 6.0;

            let visuals = &mut style.visuals;
            visuals.panel_fill = p.panel;
            visuals.window_fill = p.card;
            visuals.extreme_bg_color = rgb(0x0c0a09, 0xe7e5e4);
            visuals.faint_bg_color = rgb(0x24201d, 0xf5f5f4);
            visuals.selection.bg_fill = p.accent;
            visuals.selection.stroke = Stroke::new(1.0_f32, p.on_accent);
            visuals.hyperlink_color = p.accent;
            visuals.slider_trailing_fill = true;
            visuals.window_corner_radius = CornerRadius::same(12);
            visuals.menu_corner_radius = CornerRadius::same(10);
            let widgets = &mut visuals.widgets;
            for (state, fill) in [
                (&mut widgets.inactive, rgb(0x292524, 0xe7e5e4)),
                (&mut widgets.hovered, rgb(0x33302d, 0xd6d3d1)),
                (&mut widgets.active, rgb(0x44403c, 0xc9c5c2)),
                (&mut widgets.open, rgb(0x33302d, 0xd6d3d1)),
            ] {
                state.weak_bg_fill = fill;
                state.bg_fill = fill;
                state.corner_radius = CornerRadius::same(8);
            }
            widgets.noninteractive.corner_radius = CornerRadius::same(8);
            widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
            widgets.inactive.bg_stroke = Stroke::NONE;
            widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent.gamma_multiply(0.6));
            widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
        });
    }
}

fn theme_from_name(name: &str) -> egui::ThemePreference {
    match name {
        "light" => egui::ThemePreference::Light,
        "system" => egui::ThemePreference::System,
        _ => egui::ThemePreference::Dark,
    }
}

fn theme_switcher(ui: &mut egui::Ui) {
    let current = ui.ctx().options(|o| o.theme_preference);
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .corner_radius(10)
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (preference, label, name) in [
                    (egui::ThemePreference::Dark, "Tối", "dark"),
                    (egui::ThemePreference::Light, "Sáng", "light"),
                    (egui::ThemePreference::System, "Theo hệ thống", "system"),
                ] {
                    if ui.selectable_label(current == preference, label).clicked() && current != preference {
                        ui.ctx().set_theme(preference);
                        ibasso_control::settings::save_theme(name);
                    }
                }
            });
        });
}

/// egui's bundled fonts lack some Vietnamese glyphs, so a system font is added when one exists.
fn load_system_font(ctx: &egui::Context) {
    const CANDIDATES: [&str; 4] = [
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    ];
    let Some(bytes) = CANDIDATES.iter().find_map(|path| std::fs::read(path).ok()) else { return };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("system".into(), egui::FontData::from_owned(bytes).into());
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "system".into());
    }
    ctx.set_fonts(fonts);
}

impl App {
    fn update(&mut self, ctx: &egui::Context) {
        self.pump();
        let p = if ctx.style().visuals.dark_mode { DARK } else { LIGHT };
        ctx.request_repaint_after(Duration::from_millis(200));
        if let Some(message) = self.message.clone() {
            egui::TopBottomPanel::bottom("message")
                .frame(egui::Frame::new().fill(p.message_bg).inner_margin(Margin::symmetric(16, 8)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(message).color(p.message_text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Đóng").clicked() {
                                self.message = None;
                            }
                        });
                    });
                });
        }
        egui::SidePanel::left("devices")
            .default_width(270.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                egui::TopBottomPanel::bottom("settings")
                    .frame(egui::Frame::new().inner_margin(Margin::symmetric(0, 8)))
                    .show_separator_line(false)
                    .show_inside(ui, |ui| self.footer(ui));
                egui::ScrollArea::vertical().show(ui, |ui| self.device_list(ui));
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg).inner_margin(Margin::same(20))).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if self.settings_open {
                    self.settings_page(ui);
                } else {
                    self.controls(ui);
                }
            });
        });
    }
}

/// Requests from the tray icon and from a second launch of the app.
#[derive(Default)]
struct Shared {
    window: Mutex<Option<egui::Context>>,
    /// The native window, used to show it again while it is hidden (Windows stops running the UI then).
    #[cfg(windows)]
    hwnd: std::sync::atomic::AtomicIsize,
    show: AtomicBool,
}

impl Shared {
    fn deliver(&self, signal: Signal) {
        match signal {
            Signal::Show => {
                self.show.store(true, Ordering::SeqCst);
                #[cfg(windows)]
                native_show(self.hwnd.load(Ordering::SeqCst));
                if let Some(ctx) = self.window.lock().unwrap().as_ref() {
                    ctx.request_repaint();
                }
            }
            // The tray icon is already gone and settings are saved as they change, so nothing is left to do.
            Signal::Quit => std::process::exit(0),
        }
    }
}

#[cfg(windows)]
fn native_show(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};
    if hwnd == 0 {
        return;
    }
    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
    }
}

/// Runs the app in its window. Closing the window with X hides it while a tray icon is available, so the
/// connection to the DAC stays open; only Thoát in the tray menu ends the app.
struct Window {
    app: App,
    shared: Arc<Shared>,
}

impl eframe::App for Window {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && self.app.tray {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if self.shared.show.swap(false, Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.app.update(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.app.disconnect();
    }
}

fn main() -> eframe::Result<()> {
    let background = std::env::args().any(|arg| arg == autostart::BACKGROUND_FLAG);
    if std::env::args().any(|arg| arg == instance::QUIT_FLAG) {
        // Started from the taskbar's Thoát task: ask the running copy to quit, never start a new one.
        instance::send(Signal::Quit);
        return Ok(());
    }
    #[cfg(windows)]
    ibasso_control::taskbar::set_process_identity();
    let shared = Arc::new(Shared::default());

    let on_launch = shared.clone();
    let on_request = move |signal| match signal {
        Signal::Show => on_launch.deliver(Signal::Show),
        // Goes through the tray so its icon is removed before the process ends.
        Signal::Quit => {
            if !tray::quit() {
                std::process::exit(0);
            }
        }
    };
    if !instance::claim(on_request) {
        // Another copy is running; it has been asked to show its window.
        return Ok(());
    }
    let on_tray = shared.clone();
    let tray = tray::spawn(move |signal| on_tray.deliver(signal));

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 760.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("iBasso Control")
            .with_icon(egui::IconData { rgba: icon::rgba(256), width: 256, height: 256 }),
        ..Default::default()
    };
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_some() {
        // Wayland ignores requests to hide a window, so use X11 (XWayland on Wayland desktops) when it exists.
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11();
        }));
    }
    #[cfg(not(target_os = "linux"))]
    let _ = &mut options;

    // Started at login: stay in the tray. Without a tray the window must show, or there is no way back.
    let hidden = background && tray;
    eframe::run_native(
        "iBasso Control",
        options,
        Box::new(move |cc| {
            load_system_font(&cc.egui_ctx);
            apply_theme(&cc.egui_ctx);
            cc.egui_ctx.set_theme(theme_from_name(&ibasso_control::settings::load_theme().unwrap_or_default()));
            #[cfg(windows)]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if let Ok(handle) = cc.window_handle() {
                    if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                        shared.hwnd.store(win32.hwnd.get(), Ordering::SeqCst);
                        let _ = ibasso_control::taskbar::set_window_identity(win32.hwnd.get());
                        let _ = ibasso_control::taskbar::install_jump_list();
                    }
                }
            }
            *shared.window.lock().unwrap() = Some(cc.egui_ctx.clone());
            if hidden {
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
            Ok(Box::new(Window { app: App::new(tray), shared }))
        }),
    )
}
