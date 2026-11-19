//! Windows taskbar integration.
//!
//! - An explicit app identity on the window, so the taskbar names the app "iBasso Control" rather than a
//!   file name Windows may have cached from an older build, and pins relaunch the app correctly.
//! - A "Thoát" task in the jump list (the taskbar button's right-click menu). It starts the app with
//!   `--quit`, which asks the running copy to quit.
use crate::instance::QUIT_FLAG;
use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::{HWND, PROPERTYKEY};
use windows::Win32::Storage::EnhancedStorage::{
    PKEY_AppUserModel_ID, PKEY_AppUserModel_RelaunchCommand, PKEY_AppUserModel_RelaunchDisplayNameResource,
    PKEY_AppUserModel_RelaunchIconResource, PKEY_Title,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::{
    DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW,
    SetCurrentProcessExplicitAppUserModelID, ShellLink,
};

/// Identity the taskbar groups the app's windows, pins and jump list under.
pub const APP_ID: &str = "dum1pr0.iBassoControl";

const NAME: &str = "iBasso Control";

fn exe() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

fn set(store: &IPropertyStore, key: &PROPERTYKEY, value: &str) -> windows::core::Result<()> {
    unsafe { store.SetValue(key, &PROPVARIANT::from(value)) }
}

/// Must run before the first window is created.
pub fn set_process_identity() {
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID));
    }
}

/// Names the window's taskbar button and tells Windows how to start the app again from a pin.
pub fn set_window_identity(hwnd: isize) -> windows::core::Result<()> {
    let exe = exe();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let store: IPropertyStore = SHGetPropertyStoreForWindow(HWND(hwnd as *mut _))?;
        set(&store, &PKEY_AppUserModel_ID, APP_ID)?;
        set(&store, &PKEY_AppUserModel_RelaunchCommand, &format!("\"{exe}\""))?;
        set(&store, &PKEY_AppUserModel_RelaunchDisplayNameResource, NAME)?;
        set(&store, &PKEY_AppUserModel_RelaunchIconResource, &format!("{exe},0"))?;
        store.Commit()
    }
}

/// Puts "Thoát" in the taskbar button's right-click menu.
pub fn install_jump_list() -> windows::core::Result<()> {
    let exe = HSTRING::from(exe());
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        list.SetAppID(&HSTRING::from(APP_ID))?;
        let mut slots = 0u32;
        let _removed: IObjectArray = list.BeginList(&mut slots)?;

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&exe)?;
        link.SetArguments(&HSTRING::from(QUIT_FLAG))?;
        link.SetIconLocation(&exe, 0)?;
        link.SetDescription(&HSTRING::from("Thoát hẳn iBasso Control"))?;
        let title: IPropertyStore = link.cast()?;
        set(&title, &PKEY_Title, "Thoát")?;
        title.Commit()?;

        let tasks: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        tasks.AddObject(&link)?;
        list.AddUserTasks(&tasks.cast::<IObjectArray>()?)?;
        list.CommitList()
    }
}
