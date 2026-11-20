//! Linux application menu entry. It names the window's class so the taskbar groups the window under it, and
//! adds a "Thoát" action, which KDE and GNOME show when the taskbar button is right-clicked (the counterpart
//! of the Windows jump list task).
use crate::autostart::quote_exec;
use crate::instance::QUIT_FLAG;
use std::path::{Path, PathBuf};

/// Window class, which must match `StartupWMClass` for the taskbar to pair the window with the entry.
pub const APP_ID: &str = "ibasso-control";

const ICON: &[u8] = include_bytes!("../assets/icon.png");

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

pub fn entry(exe: &str) -> String {
    let exe = quote_exec(exe);
    format!(
        "[Desktop Entry]\nType=Application\nName=iBasso Control\nComment=Điều khiển DAC/amp USB iBasso\n\
         Exec={exe}\nIcon={APP_ID}\nTerminal=false\nCategories=AudioVideo;Audio;\nStartupWMClass={APP_ID}\n\
         Actions=quit;\n\n[Desktop Action quit]\nName=Thoát\nExec={exe} {QUIT_FLAG}\n"
    )
}

/// The program an existing entry starts, from its first Exec line.
fn exec_program(text: &str) -> Option<String> {
    let line = text.lines().find_map(|l| l.strip_prefix("Exec="))?.trim();
    let program = match line.strip_prefix('"') {
        Some(rest) => {
            let mut out = String::new();
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => out.extend(chars.next()),
                    '"' => break,
                    c => out.push(c),
                }
            }
            out
        }
        None => line.split_whitespace().next()?.to_string(),
    };
    Some(program)
}

/// Whether this copy of the app may write the entry: when there is none yet, or when the existing one
/// already starts this copy. A build run from a cargo `target` folder never takes over the entry.
fn may_write(existing: Option<&str>, exe: &str) -> bool {
    if Path::new(exe).components().any(|c| c.as_os_str() == "target") {
        return false;
    }
    match existing {
        None => true,
        Some(text) => exec_program(text).as_deref() == Some(exe),
    }
}

fn write_if_changed(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|current| current == content) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, content)
}

/// Installs or refreshes the menu entry and its icon for the running copy of the app.
pub fn ensure() {
    let (Some(data), Ok(exe)) = (data_home(), std::env::current_exe()) else { return };
    let Some(exe) = exe.to_str().map(str::to_string) else { return };
    let path = data.join("applications").join(format!("{APP_ID}.desktop"));
    let existing = std::fs::read_to_string(&path).ok();
    if !may_write(existing.as_deref(), &exe) {
        return;
    }
    let icon = data.join("icons/hicolor/256x256/apps").join(format!("{APP_ID}.png"));
    let _ = write_if_changed(&icon, ICON);
    let _ = write_if_changed(&path, entry(&exe).as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_has_a_quit_action_for_this_program() {
        let text = entry("/home/a b/ibasso-control");
        assert!(text.contains("Exec=\"/home/a b/ibasso-control\"\n"));
        assert!(text.contains("Actions=quit;"));
        assert!(text.contains("Exec=\"/home/a b/ibasso-control\" --quit\n"));
        assert!(text.contains("StartupWMClass=ibasso-control"));
        assert_eq!(exec_program(&text).as_deref(), Some("/home/a b/ibasso-control"));
    }

    #[test]
    fn only_takes_over_an_entry_for_the_same_program() {
        let exe = "/home/u/.local/bin/ibasso-control/ibasso-control";
        assert!(may_write(None, exe));
        assert!(may_write(Some(&format!("[Desktop Entry]\nExec={exe}\n")), exe));
        assert!(!may_write(Some("[Desktop Entry]\nExec=/opt/other/ibasso-control\n"), exe));
        assert!(!may_write(None, "/home/u/src/ibasso-control/target/release/ibasso-control"));
    }
}
