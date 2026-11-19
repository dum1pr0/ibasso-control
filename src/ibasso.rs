//! iBasso USB HID protocol, recovered from iBasso UAC 2.1.9 (com.ibasso.volume).
use std::collections::HashMap;

pub const VENDOR_IDS: [u16; 3] = [0x262A, 0x2FC6, 0x0661];

/// Models the iBasso UAC app controls, by their marketing names.
pub const MODELS: [&str; 12] = [
    "DC03 Pro", "DC04 Pro", "DC04 Ultra", "DC06", "DC06 Pro", "DC07 Pro", "DC-Elite", "DC-Tonfa", "DC-Nunchaku",
    "Macaron", "Macchiato", "D18",
];

const VOLUME_STEPS: [u8; 101] = [
    255, 155, 150, 145, 140, 135, 130, 125, 120, 115, 110, 109, 108, 107, 106, 105, 104, 103, 102, 101, 100, 99, 98,
    97, 96, 95, 94, 93, 92, 91, 90, 88, 86, 84, 82, 80, 78, 76, 74, 72, 70, 68, 66, 64, 62, 60, 58, 56, 54, 52, 50, 49,
    48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20,
    19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Dc04u,
    Macchiato,
    D18,
    Dc07,
    Dc07Pro,
    Dc09,
    Tonfa,
    Dc06,
    Dc06Pro,
    Dc03Pro,
    Dc04Pro,
    Dc08,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wire {
    Hid64,
    Hid8,
    Register,
}

impl Family {
    /// Mirrors UacManager: the USB product string selects the controller.
    pub fn from_product(product: &str) -> Option<Family> {
        let name = product.trim();
        if name == "Macaron" {
            return Some(Family::Dc08);
        }
        const NAMES: [(&str, Family); 11] = [
            ("iBasso-DC06", Family::Dc06),
            ("iBasso-DC06-Pro", Family::Dc06Pro),
            ("iBasso-DC03-Pro", Family::Dc03Pro),
            ("iBasso-DC04-Pro", Family::Dc04Pro),
            ("iBasso DC-Elite", Family::Dc07),
            ("iBasso DC-Tonfa", Family::Tonfa),
            ("iBasso DC07 Pro", Family::Dc07Pro),
            ("iBasso DC-Nunchaku", Family::Dc09),
            ("DC04U", Family::Dc04u),
            ("D18", Family::D18),
            ("Macchiato", Family::Macchiato),
        ];
        NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, f)| *f)
    }

    fn wire(self) -> Wire {
        match self {
            Family::Dc04u | Family::Macchiato | Family::D18 => Wire::Hid64,
            Family::Dc07 | Family::Dc07Pro | Family::Dc09 | Family::Tonfa => Wire::Hid8,
            _ => Wire::Register,
        }
    }

    /// False for models whose settings cannot be read back from the device.
    pub fn reports_state(self) -> bool {
        !matches!(self, Family::Dc03Pro | Family::Dc04Pro | Family::Dc08)
    }

    pub fn has_peq(self) -> bool {
        matches!(self, Family::Dc04u | Family::Macchiato)
    }

    /// Settings of this model, with the option names and values the iBasso app uses for each one.
    pub fn controls(self) -> Vec<Control> {
        use Ctl::*;
        let slider = |id, min, max| Control { id, min, max, options: &[] };
        let choice = |id, options: &'static [(i32, &'static str)]| Control {
            id,
            min: options.iter().map(|o| o.0).min().unwrap_or(0),
            max: options.iter().map(|o| o.0).max().unwrap_or(0),
            options,
        };
        match self {
            Family::Dc04u | Family::Macchiato => vec![
                slider(Volume, 0, 100), slider(Balance, 0, 20), choice(Gain, GAIN_3), choice(Filter, FILTER_AKM),
                choice(Eq, EQ_PRESETS), choice(Dre, DRE), choice(Spdif, ON_OFF), choice(AmpMode, AMP_MODE),
                slider(OpVoltage, 0, 20), choice(MicMode, ON_OFF), choice(ScreenRotation, ON_OFF),
                choice(ScreenTime, SCREEN_TIME), choice(Language, LANGUAGE),
            ],
            Family::D18 => vec![
                slider(Volume, 0, 100), slider(Balance, 0, 20), choice(Gain, GAIN_D18), choice(DacGain, GAIN_2),
                choice(Filter, FILTER_ESS), choice(Spdif, ON_OFF), choice(MicMode, ON_OFF),
                choice(ScreenRotation, ON_OFF), choice(ScreenTime, SCREEN_TIME), choice(Language, LANGUAGE),
            ],
            Family::Dc07 => vec![
                choice(Filter, FILTER_DC07), choice(PcmTrim, PCM_TRIM), choice(Spdif, ON_OFF), choice(Output, ON_OFF),
                choice(KeyControl, ON_OFF),
            ],
            Family::Dc07Pro => vec![
                slider(Volume, 0, 100), slider(Balance, 0, 20), choice(Gain, GAIN_3), choice(Filter, FILTER_AKM),
                choice(HpFilter, ON_OFF), choice(Spdif, ON_OFF), choice(KeyControl, ON_OFF),
                choice(ScreenRotation, ON_OFF), choice(ScreenTime, SCREEN_TIME),
            ],
            Family::Dc09 => vec![
                slider(Volume, 0, 100), slider(Balance, 0, 20), choice(Gain, GAIN_2), choice(Filter, FILTER_AKM),
                choice(HpFilter, ON_OFF), choice(Spdif, ON_OFF), choice(WorkMode, WORK_MODE),
                choice(KeyControl, ON_OFF), choice(ScreenRotation, ON_OFF), choice(ScreenTime, SCREEN_TIME),
            ],
            Family::Tonfa => vec![
                slider(Volume, 0, 100), slider(Balance, 0, 20), choice(Gain, GAIN_2), choice(Filter, FILTER_AKM),
                choice(Spdif, ON_OFF), choice(KeyControl, ON_OFF), choice(ScreenRotation, ON_OFF),
                choice(ScreenTime, SCREEN_TIME),
            ],
            Family::Dc06 => vec![slider(Volume, 0, 100), slider(Balance, -10, 10), choice(Filter, FILTER_DC06)],
            Family::Dc06Pro => vec![
                slider(Volume, 0, 100), slider(Balance, -10, 10), choice(Filter, FILTER_DC06), choice(Spdif, ON_OFF),
            ],
            Family::Dc03Pro => vec![
                slider(Volume, 0, 100), slider(Balance, -10, 10), choice(Gain, GAIN_3), choice(Filter, FILTER_REGISTER),
                choice(Output, OUTPUT),
            ],
            Family::Dc04Pro | Family::Dc08 => vec![
                slider(Volume, 0, 100), slider(Balance, -10, 10), choice(Gain, GAIN_3), choice(Filter, FILTER_REGISTER),
                choice(Output, OUTPUT), choice(Spdif, ON_OFF),
            ],
        }
    }
}

// Option names and values, taken from the iBasso app's layouts and fragments.
const ON_OFF: &[(i32, &str)] = &[(0, "Tắt"), (1, "Bật")];
const GAIN_2: &[(i32, &str)] = &[(0, "Low"), (1, "High")];
const GAIN_3: &[(i32, &str)] = &[(0, "Low"), (1, "Medium"), (2, "High")];
const GAIN_D18: &[(i32, &str)] = &[(0, "Low"), (1, "High"), (2, "Super")];
const FILTER_AKM: &[(i32, &str)] = &[
    (0, "D1 · Phase-compensated Fast"),
    (1, "D2 · Phase-compensated Slow"),
    (2, "D3 · Low-latency Fast"),
    (3, "D4 · Low-latency Slow"),
    (4, "D5 · NOS"),
];
const FILTER_ESS: &[(i32, &str)] = &[
    (0, "D1 · Fast Roll Off"),
    (1, "D2 · Short Delay Slow Roll Off"),
    (2, "D3 · Short Delay Fast Roll Off"),
    (3, "D4 · Slow Roll Off"),
    (4, "D5 · NOS"),
];
/// DC03 Pro, DC04 Pro and Macaron send the DAC register value itself, in this order.
const FILTER_REGISTER: &[(i32, &str)] = &[
    (66, "D1 · Fast Roll Off"),
    (130, "D2 · Short Delay Slow Roll Off"),
    (2, "D3 · Short Delay Fast Roll Off"),
    (194, "D4 · Slow Roll Off"),
    (34, "D5 · NOS"),
];
const FILTER_DC06: &[(i32, &str)] = &[(0, "D1 · Linear Phase Fast Roll-off"), (192, "D2 · Hybrid Fast Roll-off")];
const FILTER_DC07: &[(i32, &str)] = &[(0, "D1 · Sharp Roll-Off"), (1, "D2 · Slow Roll-Off")];
const EQ_PRESETS: &[(i32, &str)] = &[
    (0, "Tắt"), (1, "Jazz"), (2, "Pop"), (3, "Rock"), (4, "Dance"), (5, "R&B"), (6, "Classical"), (7, "Hip-hop"),
    (8, "Custom 1"), (9, "Custom 2"), (10, "Custom 3"),
];
const DRE: &[(i32, &str)] = &[(0, "Tắt"), (1, "Bật"), (2, "Optimization")];
const AMP_MODE: &[(i32, &str)] = &[(0, "Class AB"), (1, "Class H")];
const SCREEN_TIME: &[(i32, &str)] = &[(0, "10 giây"), (1, "20 giây"), (2, "30 giây"), (3, "Luôn sáng")];
const LANGUAGE: &[(i32, &str)] = &[(0, "Tiếng Anh"), (1, "Tiếng Trung"), (2, "Tiếng Nhật")];
const PCM_TRIM: &[(i32, &str)] = &[(0, "0 dB"), (1, "-1 dB"), (2, "-2 dB"), (3, "-3 dB")];
const OUTPUT: &[(i32, &str)] = &[(0, "Normal"), (1, "Turbo")];
const WORK_MODE: &[(i32, &str)] = &[(0, "Tube"), (1, "AB")];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ctl {
    Volume,
    Balance,
    Gain,
    Filter,
    Eq,
    Dre,
    Spdif,
    AmpMode,
    MicMode,
    ScreenRotation,
    ScreenTime,
    Language,
    HpFilter,
    PcmTrim,
    Output,
    KeyControl,
    OpVoltage,
    DacGain,
    WorkMode,
}

impl Ctl {
    pub const ALL: [Ctl; 19] = [
        Ctl::Volume, Ctl::Balance, Ctl::Gain, Ctl::Filter, Ctl::Eq, Ctl::Dre, Ctl::Spdif, Ctl::AmpMode, Ctl::MicMode,
        Ctl::ScreenRotation, Ctl::ScreenTime, Ctl::Language, Ctl::HpFilter, Ctl::PcmTrim, Ctl::Output,
        Ctl::KeyControl, Ctl::OpVoltage, Ctl::DacGain, Ctl::WorkMode,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Ctl::Volume => "Âm lượng",
            Ctl::Balance => "Cân bằng trái/phải",
            Ctl::Gain => "Gain",
            Ctl::Filter => "Bộ lọc số",
            Ctl::Eq => "Equalizer",
            Ctl::Dre => "DRE",
            Ctl::Spdif => "Coax (S/PDIF)",
            Ctl::AmpMode => "Nguồn analog DAC",
            Ctl::MicMode => "Micro",
            Ctl::ScreenRotation => "Xoay màn hình 180°",
            Ctl::ScreenTime => "Thời gian sáng màn hình",
            Ctl::Language => "Ngôn ngữ trên máy",
            Ctl::HpFilter => "Bộ lọc tai nghe",
            Ctl::PcmTrim => "PCM trim",
            Ctl::Output => "Ngõ ra",
            Ctl::KeyControl => "Nút âm lượng trên máy",
            Ctl::OpVoltage => "Điện áp ampli",
            Ctl::DacGain => "DAC gain",
            Ctl::WorkMode => "Chế độ ampli",
        }
    }

    pub fn from_name(name: &str) -> Option<Ctl> {
        Ctl::ALL.into_iter().find(|c| format!("{c:?}") == name)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Control {
    pub id: Ctl,
    pub min: i32,
    pub max: i32,
    /// Named choices with the value each sends. Empty for sliders.
    pub options: &'static [(i32, &'static str)],
}

impl Control {
    pub fn accepts(&self, value: i32) -> bool {
        if self.options.is_empty() {
            (self.min..=self.max).contains(&value)
        } else {
            self.options.iter().any(|(v, _)| *v == value)
        }
    }

    pub fn option_label(&self, value: i32) -> Option<&'static str> {
        self.options.iter().find(|(v, _)| *v == value).map(|(_, label)| *label)
    }

    /// Text shown next to a slider value.
    pub fn describe(&self, value: i32) -> String {
        match self.id {
            // The iBasso app sends (volts × 10) − 40 for 4.0 V to 6.0 V.
            Ctl::OpVoltage => format!("{:.1} V", (value + 40) as f32 / 10.0),
            Ctl::Balance => {
                let center = (self.min + self.max) / 2;
                match value - center {
                    0 => "L=R".to_string(),
                    offset => format!("{offset:+}"),
                }
            }
            _ => value.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// HID SET_REPORT over the control pipe.
    Control,
    /// HID interrupt OUT endpoint.
    Interrupt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub via: Via,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PeqBand {
    pub filter_type: u8,
    pub frequency: u16,
    pub gain: f64,
    pub q: f64,
}

impl Default for PeqBand {
    fn default() -> Self {
        PeqBand { filter_type: 2, frequency: 1000, gain: 0.0, q: 1.0 }
    }
}

const G_FILTER: u8 = 32;
const G_OUTPUT: u8 = 48;
const G_SCREEN: u8 = 64;
const G_USB: u8 = 80;

fn hid64_frame(cmd: u8, group: u8, payload: &[u8]) -> Frame {
    let mut b = vec![0u8; 64];
    b[..6].copy_from_slice(&[75, 1, 63, 8, cmd, group]);
    b[6..6 + payload.len()].copy_from_slice(payload);
    Frame { via: Via::Interrupt, bytes: b }
}

fn hid64_read(seq: u8, group: u8) -> Frame {
    let mut b = vec![0u8; 64];
    b[..6].copy_from_slice(&[75, 0x80, 63, 8, seq, group]);
    Frame { via: Via::Interrupt, bytes: b }
}

fn hid8(bytes: [u8; 8]) -> Frame {
    Frame { via: Via::Interrupt, bytes: bytes.to_vec() }
}

fn reg_frame(head: &[u8]) -> Frame {
    let mut b = vec![0u8; 16];
    b[..head.len()].copy_from_slice(head);
    Frame { via: Via::Control, bytes: b }
}

fn write1(seq: u8, addr: u8, off: u8, val: u8) -> Frame {
    reg_frame(&[seq, 17, 0x98, addr, off, 0, 1, val])
}
fn read1(seq: u8, addr: u8, off: u8) -> Frame {
    reg_frame(&[seq, 18, 0xDC, addr, off, 0, 1])
}
fn write2(seq: u8, addr: u8, o1: u8, o2: u8, val: u8) -> Frame {
    reg_frame(&[seq, 17, 0xA0, addr, o1, o2, 1, val])
}
fn write4(seq: u8, addr: u8, off: [u8; 4], val: u8) -> Frame {
    reg_frame(&[seq, 17, 0x88, addr, 0, 0, 5, off[0], off[1], off[2], off[3], val])
}

/// Live state for one connected device. Grouped settings are only written after the device reported them,
/// because each write replaces every field in the group.
pub struct Session {
    pub family: Family,
    pub values: HashMap<Ctl, i32>,
    pub firmware: Option<String>,
    pub peq: [[PeqBand; 10]; 3],
    pub peq_gain: [i8; 3],
    peq_seen: u32,
    groups: HashMap<u8, [u8; 5]>,
    pending_filter: Option<u8>,
}

impl Session {
    pub fn new(family: Family) -> Self {
        Session {
            family,
            values: HashMap::new(),
            firmware: None,
            peq: [[PeqBand::default(); 10]; 3],
            peq_gain: [0; 3],
            peq_seen: 0,
            groups: HashMap::new(),
            pending_filter: None,
        }
    }

    /// True once all 30 bands were read, so saving cannot overwrite a slot with placeholder values.
    pub fn peq_ready(&self) -> bool {
        self.peq_seen == (1 << 30) - 1
    }

    /// Queries sent after connecting. They only read, so connecting never changes the volume.
    pub fn init_frames(&self) -> Vec<Frame> {
        match self.family.wire() {
            Wire::Hid64 => {
                let mut out = vec![
                    hid64_read(88, 0xF0), hid64_read(66, 16), hid64_read(89, G_FILTER), hid64_read(96, G_OUTPUT),
                    hid64_read(97, G_SCREEN), hid64_read(99, G_USB),
                ];
                if self.family.has_peq() {
                    out.extend((0..3).flat_map(peq_read_frames));
                }
                out
            }
            Wire::Hid8 => {
                let mut out = vec![hid8([88, 0xF1, 14, 0, 0, 0, 0, 0]), hid8([89, 33, 0xDE, 0, 0, 0, 0, 0])];
                if self.family == Family::Dc07 {
                    out.push(hid8([98, 49, 0xCE, 0, 0, 0, 0, 0]));
                } else {
                    out.push(hid8([66, 17, 0xEE, 0, 0, 0, 0, 0]));
                    out.push(hid8([96, 49, 0xCE, 0, 0, 0, 0, 0]));
                    out.push(hid8([97, 65, 0xBE, 0, 0, 0, 0, 0]));
                }
                out
            }
            Wire::Register => match self.family {
                Family::Dc06 | Family::Dc06Pro => vec![read1(65, 0x90, 16)],
                // A real DC03 Pro answers DAC register reads with filler bytes and keeps only a copy of what the
                // vendor app last wrote, which is 0 after power-up. The vendor app never reads these models, so
                // their settings stay unknown until the user sets them.
                _ => Vec::new(),
            },
        }
    }

    fn slot(&self, ctl: Ctl) -> Option<(u8, usize)> {
        use Ctl::*;
        let d18 = self.family == Family::D18;
        match self.family.wire() {
            Wire::Hid64 => Some(match ctl {
                Filter => (G_FILTER, 0),
                Eq => (G_FILTER, 1),
                Dre => (G_FILTER, 2),
                Spdif => (G_OUTPUT, 0),
                Balance => (G_OUTPUT, 1),
                Gain => (G_OUTPUT, if d18 { 3 } else { 2 }),
                DacGain if d18 => (G_OUTPUT, 2),
                OpVoltage if !d18 => (G_SCREEN, 1),
                ScreenRotation => (G_SCREEN, 0),
                ScreenTime => (G_SCREEN, 2),
                Language => (G_SCREEN, 3),
                AmpMode => (G_USB, 1),
                MicMode => (G_USB, 2),
                _ => return None,
            }),
            Wire::Hid8 => Some(match (self.family, ctl) {
                (_, Filter) => (G_FILTER, 0),
                (_, HpFilter) => (G_FILTER, 1),
                (_, Spdif) => (G_OUTPUT, 0),
                (Family::Dc07, PcmTrim) => (G_OUTPUT, 1),
                (Family::Dc07, Output) => (G_OUTPUT, 2),
                (Family::Dc07, KeyControl) => (G_OUTPUT, 3),
                (_, Balance) => (G_OUTPUT, 1),
                (_, Gain) => (G_OUTPUT, 2),
                (_, ScreenRotation) => (G_SCREEN, 0),
                (_, KeyControl) => (G_SCREEN, 1),
                (_, ScreenTime) => (G_SCREEN, 2),
                (Family::Dc09, WorkMode) => (G_SCREEN, 3),
                _ => return None,
            }),
            Wire::Register => None,
        }
    }

    /// Builds the frames for one setting change.
    pub fn set(&mut self, ctl: Ctl, value: i32) -> Result<Vec<Frame>, String> {
        let control = self.family.controls().into_iter().find(|c| c.id == ctl).ok_or("Thiết bị không có mục này")?;
        if !control.accepts(value) {
            return Err("Giá trị không hợp lệ".into());
        }
        let frames = match self.family.wire() {
            Wire::Hid64 if ctl == Ctl::Volume => vec![hid64_frame(1, 16, &[value as u8, value as u8])],
            Wire::Hid8 if ctl == Ctl::Volume => {
                vec![hid8([1, 16, 0xEF, value as u8, value as u8, 0, 0, 0])]
            }
            Wire::Hid64 | Wire::Hid8 => self.set_grouped(ctl, value)?,
            Wire::Register => self.set_register(ctl, value)?,
        };
        self.values.insert(ctl, value);
        Ok(frames)
    }

    fn set_grouped(&mut self, ctl: Ctl, value: i32) -> Result<Vec<Frame>, String> {
        let (group, index) = self.slot(ctl).ok_or("Thiết bị không có mục này")?;
        let mut fields = *self.groups.get(&group).ok_or("Đang chờ thiết bị báo trạng thái, hãy thử lại")?;
        fields[index] = value as u8;
        self.groups.insert(group, fields);
        Ok(vec![match self.family.wire() {
            Wire::Hid64 => hid64_frame(if group == G_FILTER { 17 } else if group == G_USB { 33 } else { 25 }, group, &fields),
            _ => {
                let (cmd, tag, check) = match group {
                    G_FILTER => (17, 32, 0xDF),
                    G_OUTPUT => (25, 48, 0xCF),
                    _ => (25, 64, 0xBF),
                };
                hid8([cmd, tag, check, 0, fields[0], fields[1], fields[2], fields[3]])
            }
        }])
    }

    fn volume_pair(&self, index: usize, balance: i32) -> (u8, u8) {
        let raw = VOLUME_STEPS[index] as i32;
        let reduced = |delta: i32| if raw == 255 { 255 } else { (raw - delta).max(0) };
        if balance >= 0 {
            (raw as u8, reduced(balance) as u8)
        } else {
            (reduced(-balance) as u8, raw as u8)
        }
    }

    fn register_volume(&self, seq: [u8; 8], left: u8, right: u8) -> Vec<Frame> {
        let [a, b, c, d, e, f, g, h] = seq;
        match self.family {
            Family::Dc06 => vec![
                write1(a, 0x90, 16, left), write1(b, 0x90, 15, left), write1(e, 0x92, 16, right), write1(f, 0x92, 15, right),
            ],
            Family::Dc06Pro => vec![
                write1(a, 0x90, 16, left), write1(b, 0x90, 15, left), write1(e, 0x92, 16, right), write1(f, 0x92, 15, right),
                write2(19, 0xA2, 0, 16, left), write2(20, 0xA2, 0, 17, right),
            ],
            _ => vec![
                write4(a, 0x60, [9, 0, 1, 0], left), write4(b, 0x60, [9, 0, 2, 0], left),
                write4(e, 0x62, [9, 0, 1, 0], right), write4(f, 0x62, [9, 0, 2, 0], right),
                write4(c, 0x60, [7, 0, 0, 0], left), write4(d, 0x60, [7, 0, 1, 0], left), write2(19, 0xA2, 0, 16, left),
                write4(g, 0x62, [7, 0, 0, 0], right), write4(h, 0x62, [7, 0, 1, 0], right), write2(20, 0xA2, 0, 17, right),
            ],
        }
    }

    fn set_register(&mut self, ctl: Ctl, value: i32) -> Result<Vec<Frame>, String> {
        let dc06 = matches!(self.family, Family::Dc06 | Family::Dc06Pro);
        Ok(match ctl {
            Ctl::Volume => {
                let balance = *self.values.get(&Ctl::Balance).unwrap_or(&0);
                let (l, r) = self.volume_pair(value as usize, balance);
                let seq = match balance {
                    0 => [1, 2, 9, 10, 3, 4, 11, 12],
                    b if b > 0 => [1, 2, 9, 10, 7, 8, 15, 16],
                    _ => [5, 6, 13, 14, 3, 4, 11, 12],
                };
                self.register_volume(seq, l, r)
            }
            Ctl::Balance => {
                let index = *self.values.get(&Ctl::Volume).ok_or("Đang chờ thiết bị báo âm lượng, hãy thử lại")?;
                let (l, r) = self.volume_pair(index as usize, value);
                self.register_volume([5, 6, 13, 14, 7, 8, 15, 16], l, r)
            }
            // Filter values are DAC register bits here, already validated against the model's option list.
            Ctl::Filter if dc06 => {
                self.pending_filter = Some(value as u8);
                vec![read1(71, 0x90, 7), read1(72, 0x92, 7)]
            }
            Ctl::Filter => {
                let v = value as u8;
                vec![write4(17, 0x60, [9, 0, 0, 0], v), write4(18, 0x62, [9, 0, 0, 0], v)]
            }
            Ctl::Gain => {
                let v = [0u8, 32, 49][value as usize];
                vec![write4(21, 0x60, [8, 0, 0, 0], v), write4(22, 0x62, [8, 0, 0, 0], v)]
            }
            Ctl::Output => {
                let v = if value == 1 { 30 } else { 28 };
                vec![write4(23, 0x60, [11, 0, 0, 0], v), write4(24, 0x62, [11, 0, 0, 0], v)]
            }
            Ctl::Spdif => {
                let on = if self.family == Family::Dc06Pro { value == 0 } else { value == 1 };
                let v = if on { 32 } else { 0 };
                vec![
                    reg_frame(&[25, 17, 0x98, 0x2C, 2, 0, 1, v]),
                    write2(32, 0xA2, 0, 18, v),
                ]
            }
            _ => return Err("Thiết bị không có mục này".into()),
        })
    }

    fn store_group(&mut self, group: u8, fields: [u8; 5]) {
        self.groups.insert(group, fields);
        let controls = self.family.controls();
        for c in controls {
            if let Some((g, i)) = self.slot(c.id) {
                if g == group {
                    self.values.insert(c.id, fields[i] as i32);
                }
            }
        }
    }

    fn store_raw_volume(&mut self, raw: u8) {
        let index = VOLUME_STEPS.iter().position(|&s| s <= raw).unwrap_or(100);
        self.values.insert(Ctl::Volume, index as i32);
    }

    /// True when `report` answers the query in `frame`, using the same fields the vendor app matches on.
    pub fn answers(&self, frame: &Frame, report: &[u8]) -> bool {
        let q = &frame.bytes;
        match self.family.wire() {
            Wire::Hid64 => report.len() >= 64 && report[0] == 75 && report[1] == 0x80 && report[4..6] == q[4..6],
            Wire::Hid8 => report.len() >= 8 && report[0] == q[0],
            Wire::Register => report.len() > 8 && report[6] == q[0],
        }
    }

    /// Applies one input report and returns any follow-up frames.
    pub fn on_report(&mut self, r: &[u8]) -> Vec<Frame> {
        match self.family.wire() {
            Wire::Hid64 if r.len() >= 64 => self.on_hid64(r),
            Wire::Hid8 if r.len() >= 8 => self.on_hid8(r),
            Wire::Register if r.len() > 8 => return self.on_register(r),
            _ => {}
        }
        Vec::new()
    }

    fn on_hid64(&mut self, r: &[u8]) {
        if r[..4] == [75, 0x80, 9, 31] {
            let (slot, band) = (r[4].wrapping_sub(7) as usize, r[5] as usize);
            if slot < 3 && band < 10 {
                self.peq[slot][band] = PeqBand {
                    filter_type: r[34],
                    frequency: u16::from_le_bytes([r[28], r[29]]),
                    q: u16::from_le_bytes([r[30], r[31]]) as f64 / 256.0,
                    gain: i16::from_le_bytes([r[32], r[33]]) as f64 / 256.0,
                };
                self.peq_gain[slot] = r[35] as i8;
                self.peq_seen |= 1 << (slot * 10 + band);
            }
            return;
        }
        if r[4] == 88 {
            let build = if r[8] == 0 { "30".to_string() } else { format!("{:x}", r[8]) };
            self.firmware = Some(format!("{}.{:02}.{}", r[6], r[7], build));
            return;
        }
        match r[5] {
            16 => {
                self.values.insert(Ctl::Volume, r[6] as i32);
            }
            g @ (G_FILTER | G_OUTPUT | G_SCREEN | G_USB) => {
                self.store_group(g, [r[6], r[7], r[8], r[9], r[10]]);
            }
            _ => {}
        }
    }

    fn on_hid8(&mut self, r: &[u8]) {
        let fields = [r[4], r[5], r[6], r[7], 0];
        match (r[0], r[1]) {
            (88, _) => {
                if r[4] >> 4 == 0 {
                    self.firmware = Some(format!("1.{}.{:x}", r[5], r[6]));
                }
            }
            (89, _) | (0, 34) => self.store_group(G_FILTER, fields),
            (96, _) | (98, _) | (0, 50) => self.store_group(G_OUTPUT, fields),
            (97, _) | (0, 66) => self.store_group(G_SCREEN, fields),
            (66, _) | (0, 18) => {
                self.values.insert(Ctl::Volume, r[4] as i32);
            }
            _ => {}
        }
    }

    fn on_register(&mut self, r: &[u8]) -> Vec<Frame> {
        if r[4] == 0xFE && r[5] == 1 {
            self.store_raw_volume(r[8]);
            return Vec::new();
        }
        match r[6] {
            65 | 66 | 69 | 70 => self.store_raw_volume(r[8]),
            seq @ (71 | 72) => {
                if let Some(filter) = self.pending_filter {
                    let value = (r[8] & 31) | filter;
                    if seq == 72 {
                        self.pending_filter = None;
                    }
                    return vec![if seq == 71 { write1(17, 0x90, 7, value) } else { write1(18, 0x92, 7, value) }];
                }
            }
            _ => {}
        }
        Vec::new()
    }
}

/// Requests the ten bands of one PEQ slot (0..=2).
pub fn peq_read_frames(slot: u8) -> Vec<Frame> {
    (0..10)
        .map(|band| {
            let mut b = vec![0u8; 64];
            b[..6].copy_from_slice(&[75, 0x80, 9, 31, slot + 7, band]);
            Frame { via: Via::Interrupt, bytes: b }
        })
        .collect()
}

/// Writes one PEQ slot, saves it, then activates it, in the order the vendor app uses.
pub fn peq_write_frames(slot: u8, bands: &[PeqBand; 10], total_gain: i8) -> Vec<Frame> {
    const TEMPLATE: [u8; 28] = [
        75, 1, 9, 31, 0, 0, 0, 0, 0x8F, 89, 7, 64, 0x95, 0xAF, 74, 0x80, 0xC4, 59, 0xA6, 63, 107, 80, 0xB5, 127, 0xAD,
        106, 74, 0xC0,
    ];
    let mut out = Vec::with_capacity(12);
    for (i, band) in bands.iter().enumerate() {
        let mut b = vec![0u8; 64];
        b[..28].copy_from_slice(&TEMPLATE);
        b[4] = slot + 7;
        b[5] = i as u8;
        b[28..30].copy_from_slice(&band.frequency.to_le_bytes());
        b[30..32].copy_from_slice(&((band.q * 256.0).round() as i16).to_le_bytes());
        b[32..34].copy_from_slice(&((band.gain * 256.0).round() as i16).to_le_bytes());
        b[34] = band.filter_type;
        b[35] = total_gain as u8;
        out.push(Frame { via: Via::Interrupt, bytes: b });
    }
    let mut save = vec![0u8; 64];
    save[..8].copy_from_slice(&[75, 1, 10, 4, slot + 7, 0, 1, 1]);
    out.push(Frame { via: Via::Interrupt, bytes: save });
    let mut apply = vec![0u8; 64];
    apply[..7].copy_from_slice(&[75, 1, 1, 4, 0, 0, 2]);
    out.push(Frame { via: Via::Interrupt, bytes: apply });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report64(group: u8, fields: [u8; 5]) -> Vec<u8> {
        let mut r = vec![0u8; 64];
        r[..6].copy_from_slice(&[75, 0x80, 63, 8, 0, group]);
        r[6..11].copy_from_slice(&fields);
        r
    }

    #[test]
    fn product_strings_select_the_vendor_controller() {
        assert_eq!(Family::from_product("dc04u"), Some(Family::Dc04u));
        assert_eq!(Family::from_product("Macaron"), Some(Family::Dc08));
        assert_eq!(Family::from_product("macaron"), None);
    }

    #[test]
    fn dc04u_volume_matches_vendor_frame() {
        let frames = Session::new(Family::Dc04u).set(Ctl::Volume, 42).unwrap();
        assert_eq!(frames[0].via, Via::Interrupt);
        assert_eq!(frames[0].bytes.len(), 64);
        assert_eq!(frames[0].bytes[..8], [75, 1, 63, 8, 1, 16, 42, 42]);
    }

    #[test]
    fn grouped_write_waits_for_device_state_and_keeps_other_fields() {
        let mut s = Session::new(Family::Dc04u);
        assert!(s.set(Ctl::Gain, 1).is_err());
        s.on_report(&report64(G_OUTPUT, [1, 12, 0, 9, 9]));
        let frames = s.set(Ctl::Gain, 2).unwrap();
        assert_eq!(frames[0].bytes[..9], [75, 1, 63, 8, 25, 48, 1, 12, 2]);
    }

    #[test]
    fn d18_gain_uses_its_own_slot() {
        let mut s = Session::new(Family::D18);
        s.on_report(&report64(G_OUTPUT, [0, 10, 3, 1, 0]));
        assert_eq!(s.set(Ctl::Gain, 2).unwrap()[0].bytes[6..10], [0, 10, 3, 2]);
    }

    #[test]
    fn eight_byte_family_matches_vendor_frames() {
        let mut s = Session::new(Family::Dc07Pro);
        assert_eq!(s.set(Ctl::Volume, 50).unwrap()[0].bytes, [1, 16, 0xEF, 50, 50, 0, 0, 0]);
        s.on_report(&[96, 0, 0, 0, 1, 10, 2, 0]);
        assert_eq!(s.set(Ctl::Gain, 1).unwrap()[0].bytes, [25, 48, 0xCF, 0, 1, 10, 1, 0]);
    }

    #[test]
    fn dc06_volume_writes_both_channels() {
        let frames = Session::new(Family::Dc06).set(Ctl::Volume, 100).unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].via, Via::Control);
        assert_eq!(frames[0].bytes[..8], [1, 17, 0x98, 0x90, 16, 0, 1, 0]);
        assert_eq!(frames[3].bytes[..8], [4, 17, 0x98, 0x92, 15, 0, 1, 0]);
    }

    #[test]
    fn dc03_filter_uses_four_byte_register_writes() {
        // D1 sends 66, as the iBasso app does for DC03 Pro.
        let frames = Session::new(Family::Dc03Pro).set(Ctl::Filter, 66).unwrap();
        assert_eq!(frames[0].bytes[..12], [17, 17, 0x88, 0x60, 0, 0, 5, 9, 0, 0, 0, 66]);
        assert_eq!(frames[1].bytes[3], 0x62);
        assert!(Session::new(Family::Dc03Pro).set(Ctl::Filter, 1).is_err());
    }

    #[test]
    fn every_choice_has_a_label_and_matches_the_vendor_lists() {
        for family in [
            Family::Dc04u, Family::Macchiato, Family::D18, Family::Dc07, Family::Dc07Pro, Family::Dc09, Family::Tonfa,
            Family::Dc06, Family::Dc06Pro, Family::Dc03Pro, Family::Dc04Pro, Family::Dc08,
        ] {
            for control in family.controls() {
                for (value, label) in control.options {
                    assert!(control.accepts(*value) && !label.is_empty(), "{family:?} {:?}", control.id);
                }
            }
        }
        let gain = |f: Family| f.controls().into_iter().find(|c| c.id == Ctl::Gain).unwrap().options.len();
        assert_eq!((gain(Family::Dc09), gain(Family::Tonfa), gain(Family::Dc04u)), (2, 2, 3));
    }

    #[test]
    fn op_voltage_uses_the_vendor_scale() {
        let control = Family::Dc04u.controls().into_iter().find(|c| c.id == Ctl::OpVoltage).unwrap();
        assert_eq!(control.describe(5), "4.5 V");
        let mut s = Session::new(Family::Dc04u);
        s.on_report(&report64(G_SCREEN, [0, 5, 1, 0, 4]));
        assert_eq!(s.values.get(&Ctl::OpVoltage), Some(&5));
        assert_eq!(s.set(Ctl::OpVoltage, 10).unwrap()[0].bytes[4..11], [25, 64, 0, 10, 1, 0, 4]);
    }

    #[test]
    fn dc06_filter_is_read_modify_write() {
        let mut s = Session::new(Family::Dc06);
        assert_eq!(s.set(Ctl::Filter, 192).unwrap()[0].bytes[..7], [71, 18, 0xDC, 0x90, 7, 0, 1]);
        let mut reply = vec![0u8; 16];
        reply[6] = 71;
        reply[8] = 0x2A;
        assert_eq!(s.on_report(&reply)[0].bytes[..8], [17, 17, 0x98, 0x90, 7, 0, 1, 0x0A | 192]);
    }

    #[test]
    fn balance_attenuates_only_one_channel_and_keeps_mute() {
        let s = Session::new(Family::Dc06);
        assert_eq!(s.volume_pair(100, 3), (0, 0));
        assert_eq!(s.volume_pair(50, 3), (50, 47));
        assert_eq!(s.volume_pair(50, -3), (47, 50));
        assert_eq!(s.volume_pair(0, 3), (255, 255));
    }

    #[test]
    fn replies_are_matched_to_their_query() {
        let s = Session::new(Family::Dc04u);
        let query = hid64_read(97, G_SCREEN);
        let mut reply = report64(G_SCREEN, [0; 5]);
        reply[4] = 97;
        assert!(s.answers(&query, &reply));
        reply[5] = G_USB;
        assert!(!s.answers(&query, &reply));
    }

    #[test]
    fn unreadable_models_send_no_status_queries() {
        for family in [Family::Dc03Pro, Family::Dc04Pro, Family::Dc08] {
            assert!(Session::new(family).init_frames().is_empty(), "{family:?}");
        }
    }

    #[test]
    fn balance_needs_a_volume_set_in_this_session_first() {
        let mut s = Session::new(Family::Dc03Pro);
        assert!(s.set(Ctl::Balance, 2).is_err());
        s.set(Ctl::Volume, 40).unwrap();
        assert!(s.set(Ctl::Balance, 2).is_ok());
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        assert!(Session::new(Family::Dc04u).set(Ctl::Volume, 101).is_err());
    }

    #[test]
    fn peq_frames_follow_vendor_layout() {
        let mut bands = [PeqBand::default(); 10];
        bands[0] = PeqBand { filter_type: 2, frequency: 1000, gain: -3.5, q: 1.41 };
        let frames = peq_write_frames(1, &bands, -2);
        assert_eq!(frames.len(), 12);
        let b = &frames[0].bytes;
        assert_eq!(b[..6], [75, 1, 9, 31, 8, 0]);
        assert_eq!(b[8..12], [0x8F, 89, 7, 64]);
        assert_eq!(u16::from_le_bytes([b[28], b[29]]), 1000);
        assert_eq!(i16::from_le_bytes([b[32], b[33]]), -896);
        assert_eq!(b[35], 0xFE);
        assert_eq!(frames[10].bytes[..8], [75, 1, 10, 4, 8, 0, 1, 1]);
        assert_eq!(frames[11].bytes[..7], [75, 1, 1, 4, 0, 0, 2]);
    }

    #[test]
    fn peq_reports_are_decoded() {
        let mut s = Session::new(Family::Dc04u);
        let mut r = vec![0u8; 64];
        r[..6].copy_from_slice(&[75, 0x80, 9, 31, 7, 3]);
        r[28..30].copy_from_slice(&250u16.to_le_bytes());
        r[30..32].copy_from_slice(&512u16.to_le_bytes());
        r[32..34].copy_from_slice(&(-256i16).to_le_bytes());
        r[34] = 1;
        s.on_report(&r);
        assert_eq!(s.peq[0][3], PeqBand { filter_type: 1, frequency: 250, gain: -1.0, q: 2.0 });
    }
}
