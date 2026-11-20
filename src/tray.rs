//! System tray icon: "Mở" and "Thoát" in its menu, a click shows the window, and turning the mouse wheel
//! over the icon changes the volume of the connected DAC.
//!
//! Windows uses `tray-icon` on its own thread with a message loop; the wheel is caught with a low-level mouse
//! hook, because Windows does not send wheel messages to tray icons. Linux uses StatusNotifierItem over D-Bus
//! through `ksni`, whose Scroll method carries the wheel.
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Show the window, opening it again if it was closed.
    Show,
    /// Leave the app for good. Sent after the tray icon has been removed.
    Quit,
    /// Change the volume of the connected device by this many steps.
    Volume(i32),
}

/// Volume steps per wheel notch.
const STEP: i32 = 2;
/// Wheel distance of one notch, on both systems.
const NOTCH: i32 = 120;

type Handler = Arc<dyn Fn(Signal) + Send + Sync>;

static STOPPER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Removes the tray icon and then reports [`Signal::Quit`], as choosing Thoát in its menu does.
/// Returns false when no tray icon is running.
pub fn quit() -> bool {
    match STOPPER.get() {
        Some(stop) => {
            stop();
            true
        }
        None => false,
    }
}

/// Turns wheel movement into volume steps. Touchpads report many small movements, so they are added up
/// until they make a whole notch.
#[derive(Default)]
struct Wheel(i32);

impl Wheel {
    /// Adds a movement (positive is up, away from the user) and returns the volume change, if any.
    fn turn(&mut self, delta: i32) -> Option<i32> {
        self.0 += delta;
        let notches = self.0 / NOTCH;
        self.0 -= notches * NOTCH;
        (notches != 0).then_some(notches * STEP)
    }
}

/// Starts the tray icon. Returns false when the icon could not be created, for example on a desktop
/// without a system tray; the app then quits when its window is closed.
pub fn spawn(on_signal: impl Fn(Signal) + Send + Sync + 'static) -> bool {
    platform::spawn(Arc::new(on_signal))
}

#[cfg(windows)]
mod platform {
    use super::{Handler, Signal, Wheel, STOPPER};
    use std::cell::RefCell;
    use std::sync::mpsc::channel;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_MOUSEWHEEL, WM_QUIT,
    };

    thread_local! {
        /// The icon and handler the wheel hook needs; the hook runs on the tray thread.
        static HOOK: RefCell<Option<(TrayIcon, Handler, Wheel)>> = const { RefCell::new(None) };
    }

    pub fn spawn(on_signal: Handler) -> bool {
        let (ready_tx, ready_rx) = channel();
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || {
                let Some(tray) = build(on_signal.clone()) else {
                    let _ = ready_tx.send(false);
                    return;
                };
                let _ = ready_tx.send(true);
                HOOK.with(|hook| *hook.borrow_mut() = Some((tray, on_signal.clone(), Wheel::default())));
                let module = unsafe { GetModuleHandleW(std::ptr::null()) };
                let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(wheel_hook), module, 0) };
                let mut msg: MSG = unsafe { std::mem::zeroed() };
                while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                    unsafe {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                if !hook.is_null() {
                    unsafe { UnhookWindowsHookEx(hook) };
                }
                // Removing the icon before the process ends keeps a dead icon from lingering in the tray.
                HOOK.with(|hook| hook.borrow_mut().take());
                on_signal(Signal::Quit);
            })
            .is_ok()
            && ready_rx.recv().unwrap_or(false)
    }

    /// Takes wheel turns made over the tray icon and passes every other mouse event on.
    unsafe extern "system" fn wheel_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && wparam as u32 == WM_MOUSEWHEEL {
            let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
            let delta = (info.mouseData >> 16) as u16 as i16 as i32;
            let handled = HOOK.with(|hook| {
                let mut hook = hook.borrow_mut();
                let Some((tray, on_signal, wheel)) = hook.as_mut() else { return false };
                let Some(rect) = tray.rect() else { return false };
                let (x, y) = (info.pt.x as f64, info.pt.y as f64);
                let inside = x >= rect.position.x
                    && y >= rect.position.y
                    && x < rect.position.x + rect.size.width as f64
                    && y < rect.position.y + rect.size.height as f64;
                if inside {
                    if let Some(steps) = wheel.turn(delta) {
                        on_signal(Signal::Volume(steps));
                    }
                }
                inside
            });
            if handled {
                return 1;
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    fn build(on_signal: Handler) -> Option<TrayIcon> {
        let menu = Menu::new();
        let open = MenuItem::new("Mở iBasso Control", true, None);
        let quit = MenuItem::new("Thoát", true, None);
        menu.append(&open).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&quit).ok()?;

        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("iBasso Control\nLăn chuột trên icon để chỉnh âm lượng")
            .with_menu_on_left_click(false);
        if let Ok(icon) = tray_icon::Icon::from_rgba(crate::icon::rgba(32), 32, 32) {
            builder = builder.with_icon(icon);
        }
        let tray = builder.build().ok()?;

        let thread = unsafe { GetCurrentThreadId() };
        let stop = move || unsafe {
            PostThreadMessageW(thread, WM_QUIT, 0, 0);
        };
        let _ = STOPPER.set(Box::new(stop));
        let (open_id, quit_id) = (open.id().clone(), quit.id().clone());
        let menu_signal = on_signal.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == open_id {
                menu_signal(Signal::Show);
            } else if event.id == quit_id {
                stop();
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                on_signal(Signal::Show);
            }
        }));
        Some(tray)
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{Handler, Signal, Wheel, STOPPER};
    use ksni::blocking::TrayMethods;
    use std::sync::{Condvar, Mutex};

    struct Tray {
        on_signal: Handler,
        wheel: Wheel,
    }

    fn stop() {
        let (lock, wake) = &FLAG;
        *lock.lock().unwrap() = true;
        wake.notify_all();
    }

    static FLAG: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

    impl ksni::Tray for Tray {
        fn id(&self) -> String {
            "ibasso-control".into()
        }

        fn title(&self) -> String {
            "iBasso Control".into()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            [32u32, 64]
                .into_iter()
                .map(|size| {
                    // StatusNotifierItem wants ARGB in network byte order.
                    let data = crate::icon::rgba(size).chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
                    ksni::Icon { width: size as i32, height: size as i32, data }
                })
                .collect()
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip {
                title: "iBasso Control".into(),
                description: "Lăn chuột trên icon để chỉnh âm lượng".into(),
                ..Default::default()
            }
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            (self.on_signal)(Signal::Show);
        }

        fn scroll(&mut self, delta: i32, orientation: ksni::Orientation) {
            if orientation == ksni::Orientation::Vertical {
                if let Some(steps) = self.wheel.turn(delta) {
                    (self.on_signal)(Signal::Volume(steps));
                }
            }
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::StandardItem;
            vec![
                StandardItem {
                    label: "Mở iBasso Control".into(),
                    activate: Box::new(|tray: &mut Self| (tray.on_signal)(Signal::Show)),
                    ..Default::default()
                }
                .into(),
                ksni::MenuItem::Separator,
                StandardItem { label: "Thoát".into(), activate: Box::new(|_| stop()), ..Default::default() }.into(),
            ]
        }
    }

    pub fn spawn(on_signal: Handler) -> bool {
        // Fails when no tray host (StatusNotifierWatcher) is running.
        let Ok(handle) = (Tray { on_signal: on_signal.clone(), wheel: Wheel::default() }).spawn() else {
            return false;
        };
        let _ = STOPPER.set(Box::new(stop));
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || {
                let (lock, wake) = &FLAG;
                let mut stopped = lock.lock().unwrap();
                while !*stopped {
                    stopped = wake.wait(stopped).unwrap();
                }
                drop(stopped);
                // Removing the icon before the process ends keeps a dead icon from lingering in the tray.
                handle.shutdown().wait();
                on_signal(Signal::Quit);
            })
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_notches_become_volume_steps() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.turn(120), Some(STEP));
        assert_eq!(wheel.turn(-240), Some(-2 * STEP));
        // Touchpad movements add up to a notch.
        assert_eq!(wheel.turn(50), None);
        assert_eq!(wheel.turn(50), None);
        assert_eq!(wheel.turn(30), Some(STEP));
        assert_eq!(wheel.turn(-10), None);
    }
}
