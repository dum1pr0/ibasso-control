//! Core logic for ibasso-control. Protocol code is free of I/O so it can be tested without hardware.
pub mod autostart;
pub mod hid;
pub mod icon;
pub mod instance;
pub mod ibasso;
pub mod settings;
#[cfg(windows)]
pub mod taskbar;
pub mod tray;
pub mod udev;
