//! Starting the app when the user logs in, hidden in the system tray.
//!
//! - Windows: a value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, which also appears on the
//!   Startup tab of Task Manager.
//! - Linux: an XDG autostart entry in `~/.config/autostart`, read by GNOME, KDE, XFCE and most other desktops.
//!
//! Both apply only to the current user and need no administrator rights.

/// Argument added to the autostart command: start with the window hidden, showing only the tray icon.
pub const BACKGROUND_FLAG: &str = "--background";

const NAME: &str = "iBasso Control";

fn exe() -> Result<String, String> {
    let path = std::env::current_exe().map_err(|e| format!("Không tìm được đường dẫn app: {e}"))?;
    Ok(path.to_string_lossy().into_owned())
}

/// Turns starting at login on or off. Turning it on again also updates the path if the app was moved.
pub fn set(enabled: bool) -> Result<(), String> {
    if enabled {
        platform::enable(&exe()?)
    } else {
        platform::disable()
    }
}

pub fn is_enabled() -> bool {
    platform::is_enabled()
}

#[cfg(not(windows))]
pub use platform::quote_exec;

#[cfg(windows)]
mod platform {
    use super::{BACKGROUND_FLAG, NAME};
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Key(HKEY);

    impl Key {
        fn open(access: u32) -> Result<Key, String> {
            let mut key: HKEY = std::ptr::null_mut();
            let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, access, &mut key) };
            if status == ERROR_SUCCESS {
                Ok(Key(key))
            } else {
                Err(format!("Không mở được khóa registry Run (mã lỗi {status})"))
            }
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe { RegCloseKey(self.0) };
        }
    }

    pub fn enable(exe: &str) -> Result<(), String> {
        let key = Key::open(KEY_SET_VALUE)?;
        // Quoted so that a path with spaces is read as one program.
        let command = wide(&format!("\"{exe}\" {BACKGROUND_FLAG}"));
        let status = unsafe {
            RegSetValueExW(key.0, wide(NAME).as_ptr(), 0, REG_SZ, command.as_ptr().cast(), (command.len() * 2) as u32)
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("Không ghi được registry (mã lỗi {status})"))
        }
    }

    pub fn disable() -> Result<(), String> {
        let key = Key::open(KEY_SET_VALUE)?;
        match unsafe { RegDeleteValueW(key.0, wide(NAME).as_ptr()) } {
            ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
            status => Err(format!("Không xóa được mục khởi động (mã lỗi {status})")),
        }
    }

    pub fn is_enabled() -> bool {
        let Ok(key) = Key::open(KEY_QUERY_VALUE) else { return false };
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                wide(NAME).as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        status == ERROR_SUCCESS
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{BACKGROUND_FLAG, NAME};
    use std::path::PathBuf;

    fn entry_path() -> Option<PathBuf> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(config.join("autostart").join("ibasso-control.desktop"))
    }

    /// Quotes a path for the Exec key of a desktop entry, as the Desktop Entry specification describes.
    pub fn quote_exec(path: &str) -> String {
        let mut out = String::from("\"");
        for c in path.chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
        out
    }

    pub fn enable(exe: &str) -> Result<(), String> {
        let path = entry_path().ok_or("Không tìm được thư mục cấu hình (~/.config)")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Không tạo được {}: {e}", dir.display()))?;
        }
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName={NAME}\nComment=Điều khiển DAC/amp USB iBasso\nExec={} {BACKGROUND_FLAG}\nIcon=ibasso-control\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            quote_exec(exe)
        );
        std::fs::write(&path, entry).map_err(|e| format!("Không ghi được {}: {e}", path.display()))
    }

    pub fn disable() -> Result<(), String> {
        match entry_path() {
            Some(path) => match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("Không xóa được {}: {e}", path.display())),
            },
            None => Ok(()),
        }
    }

    pub fn is_enabled() -> bool {
        entry_path().is_some_and(|path| path.exists())
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn exec_paths_are_quoted_and_escaped() {
            assert_eq!(super::quote_exec("/opt/My Apps/ibasso-control"), "\"/opt/My Apps/ibasso-control\"");
            assert_eq!(super::quote_exec("/a$b"), "\"/a\\$b\"");
        }
    }
}
