//! Linux udev rules that give the logged-in user access to one USB device.

/// One rule file per device, so installing access for one device never affects another.
pub fn rule_path(vid: u16, pid: u16) -> String {
    format!("/etc/udev/rules.d/70-ibasso-control-{vid:04x}-{pid:04x}.rules")
}

pub fn rule_text(vid: u16, pid: u16) -> String {
    format!(
        "# Added by ibasso-control\n\
         SUBSYSTEM==\"usb\", ATTR{{idVendor}}==\"{vid:04x}\", ATTR{{idProduct}}==\"{pid:04x}\", MODE=\"0660\", TAG+=\"uaccess\"\n\
         KERNEL==\"hidraw*\", SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{vid:04x}\", ATTRS{{idProduct}}==\"{pid:04x}\", MODE=\"0660\", TAG+=\"uaccess\"\n"
    )
}

/// True when the rule for this device already exists.
pub fn is_installed(vid: u16, pid: u16) -> bool {
    std::path::Path::new(&rule_path(vid, pid)).exists()
}

/// Installs the rule through pkexec, or sudo when pkexec is missing, then reloads udev.
#[cfg(target_os = "linux")]
pub fn install(vid: u16, pid: u16) -> Result<(), String> {
    use std::io::Write;
    use std::process::Command;

    const SCRIPT: &str = "install -m 0644 \"$1\" \"$2\" && udevadm control --reload-rules && \
                          udevadm trigger --subsystem-match=usb --subsystem-match=hidraw";
    let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    file.write_all(rule_text(vid, pid).as_bytes()).map_err(|e| e.to_string())?;
    let source = file.path().to_string_lossy().into_owned();
    let target = rule_path(vid, pid);
    let args = ["sh", "-c", SCRIPT, "sh", source.as_str(), target.as_str()];

    let status = match Command::new("pkexec").args(args).status() {
        Ok(status) => status,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Command::new("sudo").arg("-A").args(args).status().map_err(|e| format!("Không chạy được sudo: {e}"))?
        }
        Err(e) => return Err(format!("Không chạy được pkexec: {e}")),
    };
    if status.success() {
        Ok(())
    } else {
        Err("Chưa cài được quyền: yêu cầu xác thực bị từ chối hoặc lệnh lỗi".into())
    }
}

#[cfg(not(target_os = "linux"))]
pub fn install(_vid: u16, _pid: u16) -> Result<(), String> {
    Err("Quyền udev chỉ dùng trên Linux".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_targets_only_the_given_device() {
        let text = rule_text(0x0661, 0x00AB);
        assert!(text.contains("ATTR{idVendor}==\"0661\", ATTR{idProduct}==\"00ab\""));
        assert!(text.contains("ATTRS{idVendor}==\"0661\", ATTRS{idProduct}==\"00ab\""));
        assert!(text.contains("TAG+=\"uaccess\""));
        assert!(!text.contains("0666"));
    }

    #[test]
    fn each_device_gets_its_own_file() {
        assert_eq!(rule_path(0x2FC6, 0x1), "/etc/udev/rules.d/70-ibasso-control-2fc6-0001.rules");
    }
}
