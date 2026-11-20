//! Core logic for ibasso-control. Protocol code is free of I/O so it can be tested without hardware.
pub mod autostart;
#[cfg(not(windows))]
pub mod desktop_entry;
pub mod hid;
pub mod icon;
pub mod instance;
pub mod peq_import;
pub mod ibasso;
pub mod settings;
#[cfg(windows)]
pub mod taskbar;
pub mod tray;
pub mod udev;
