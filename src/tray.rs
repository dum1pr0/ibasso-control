//! System tray icon with "Mở" and "Thoát" in its menu.
//!
//! The icon lives on its own thread. Windows needs a message loop on the thread that owns the icon,
//! and the Linux backend (StatusNotifierItem over D-Bus) runs its own worker, so the thread only waits.
use std::sync::mpsc::channel;
use std::sync::{Arc, OnceLock};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Show the window, opening it again if it was closed.
    Show,
    /// Leave the app for good. Sent after the tray icon has been removed.
    Quit,
}

static STOPPER: OnceLock<platform::Stopper> = OnceLock::new();

/// Removes the tray icon and then reports [`Signal::Quit`], as choosing Thoát in its menu does.
/// Returns false when no tray icon is running.
pub fn quit() -> bool {
    match STOPPER.get() {
        Some(stop) => {
            stop.stop();
            true
        }
        None => false,
    }
}

/// Starts the tray icon. Returns false when the icon could not be created, for example on a desktop
/// without a system tray; the app then quits when its window is closed.
pub fn spawn(on_signal: impl Fn(Signal) + Send + Sync + 'static) -> bool {
    let on_signal: Arc<dyn Fn(Signal) + Send + Sync> = Arc::new(on_signal);
    let (ready_tx, ready_rx) = channel();
    std::thread::Builder::new()
        .name("tray".into())
        .spawn(move || {
            let Some(tray) = build(on_signal.clone()) else {
                let _ = ready_tx.send(false);
                return;
            };
            let _ = ready_tx.send(true);
            platform::wait();
            // Removing the icon before the process ends keeps a dead icon from lingering in the tray.
            drop(tray);
            on_signal(Signal::Quit);
        })
        .is_ok()
        && ready_rx.recv().unwrap_or(false)
}

fn build(on_signal: Arc<dyn Fn(Signal) + Send + Sync>) -> Option<tray_icon::TrayIcon> {
    let menu = Menu::new();
    let open = MenuItem::new("Mở iBasso Control", true, None);
    let quit = MenuItem::new("Thoát", true, None);
    menu.append(&open).ok()?;
    menu.append(&PredefinedMenuItem::separator()).ok()?;
    menu.append(&quit).ok()?;

    let mut builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("iBasso Control")
        .with_title("iBasso Control")
        .with_menu_on_left_click(false);
    if let Ok(icon) = tray_icon::Icon::from_rgba(crate::icon::rgba(32), 32, 32) {
        builder = builder.with_icon(icon);
    }
    let tray = builder.build().ok()?;

    let (open_id, quit_id) = (open.id().clone(), quit.id().clone());
    let stop = platform::Stopper::current();
    let _ = STOPPER.set(stop);
    let menu_signal = on_signal.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == open_id {
            menu_signal(Signal::Show);
        } else if event.id == quit_id {
            stop.stop();
        }
    }));
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            on_signal(Signal::Show);
        }
    }));
    Some(tray)
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, PostThreadMessageW, TranslateMessage, MSG, WM_QUIT,
    };

    /// Ends the tray thread's message loop from any thread.
    #[derive(Clone, Copy)]
    pub struct Stopper(u32);

    impl Stopper {
        pub fn current() -> Self {
            Stopper(unsafe { GetCurrentThreadId() })
        }

        pub fn stop(&self) {
            unsafe { PostThreadMessageW(self.0, WM_QUIT, 0, 0) };
        }
    }

    /// Runs the message loop that delivers the icon's clicks and menu commands, until stopped.
    pub fn wait() {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::sync::{Condvar, Mutex, OnceLock};

    fn flag() -> &'static (Mutex<bool>, Condvar) {
        static FLAG: OnceLock<(Mutex<bool>, Condvar)> = OnceLock::new();
        FLAG.get_or_init(|| (Mutex::new(false), Condvar::new()))
    }

    /// Wakes the tray thread so it removes the icon and reports Quit.
    #[derive(Clone, Copy)]
    pub struct Stopper;

    impl Stopper {
        pub fn current() -> Self {
            Stopper
        }

        pub fn stop(&self) {
            let (lock, wake) = flag();
            *lock.lock().unwrap() = true;
            wake.notify_all();
        }
    }

    /// The D-Bus backend has its own worker thread, so this thread only waits for Quit.
    pub fn wait() {
        let (lock, wake) = flag();
        let mut stopped = lock.lock().unwrap();
        while !*stopped {
            stopped = wake.wait(stopped).unwrap();
        }
    }
}
