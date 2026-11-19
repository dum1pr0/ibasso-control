//! Remembers the last value set for each setting of each device, like the iBasso app does for models
//! whose settings cannot be read back (DC03 Pro, DC04 Pro, Macaron).
use crate::ibasso::Ctl;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Default)]
pub struct Settings {
    path: Option<PathBuf>,
    devices: BTreeMap<String, BTreeMap<Ctl, i32>>,
}

fn config_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    Some(base?.join("ibasso-control").join("settings.txt"))
}

/// The colour theme the user picked: "dark", "light" or "system". Stored next to the device settings.
pub fn load_theme() -> Option<String> {
    let path = config_path()?.with_file_name("theme.txt");
    Some(std::fs::read_to_string(path).ok()?.trim().to_string())
}

pub fn save_theme(theme: &str) {
    if let Some(path) = config_path().map(|p| p.with_file_name("theme.txt")) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, theme);
    }
}

/// One line per setting: device name, setting name and value, separated by tabs.
fn parse(text: &str) -> BTreeMap<String, BTreeMap<Ctl, i32>> {
    let mut devices: BTreeMap<String, BTreeMap<Ctl, i32>> = BTreeMap::new();
    for line in text.lines() {
        let mut parts = line.split('\t');
        let (Some(device), Some(name), Some(value)) = (parts.next(), parts.next(), parts.next()) else { continue };
        if let (Some(ctl), Ok(value)) = (Ctl::from_name(name), value.trim().parse()) {
            devices.entry(device.to_string()).or_default().insert(ctl, value);
        }
    }
    devices
}

fn format(devices: &BTreeMap<String, BTreeMap<Ctl, i32>>) -> String {
    let mut out = String::new();
    for (device, values) in devices {
        for (ctl, value) in values {
            out.push_str(&format!("{}\t{ctl:?}\t{value}\n", device.replace(['\t', '\n'], " ")));
        }
    }
    out
}

impl Settings {
    pub fn load() -> Settings {
        let path = config_path();
        let devices = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse(&t)).unwrap_or_default();
        Settings { path, devices }
    }

    /// Saved values for one device, volume first so that balance can be applied after it.
    pub fn for_device(&self, device: &str) -> Vec<(Ctl, i32)> {
        let Some(values) = self.devices.get(device) else { return Vec::new() };
        Ctl::ALL.iter().filter_map(|ctl| values.get(ctl).map(|v| (*ctl, *v))).collect()
    }

    pub fn remember(&mut self, device: &str, ctl: Ctl, value: i32) {
        self.devices.entry(device.to_string()).or_default().insert(ctl, value);
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, format(&self.devices));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_survive_a_save_and_load() {
        let mut devices = BTreeMap::new();
        devices.entry("iBasso-DC03-Pro".to_string()).or_insert_with(BTreeMap::new).insert(Ctl::Volume, 73);
        devices.get_mut("iBasso-DC03-Pro").unwrap().insert(Ctl::Filter, 66);
        assert_eq!(parse(&format(&devices)), devices);
    }

    #[test]
    fn broken_lines_are_skipped() {
        let devices = parse("DC04U\tVolume\t35\nbad line\nDC04U\tNoSuchSetting\t1\nDC04U\tGain\tx\n");
        assert_eq!(devices["DC04U"].len(), 1);
    }

    #[test]
    fn volume_comes_before_balance() {
        let mut settings = Settings::default();
        settings.remember("DC03", Ctl::Balance, 2);
        settings.remember("DC03", Ctl::Volume, 40);
        assert_eq!(settings.for_device("DC03"), [(Ctl::Volume, 40), (Ctl::Balance, 2)]);
    }
}
