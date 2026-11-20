//! Keeps a single running copy of the app. Opening the app again while it runs in the tray shows the
//! existing window instead of starting a second copy that would fight over the DAC, and the taskbar's
//! "Thoát" task asks the running copy to quit.
//!
//! The first copy listens on a fixed loopback port; later copies connect, send one request and exit.
use crate::tray::Signal;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

/// Argument that asks the running copy to quit (used by the taskbar's Thoát task).
pub const QUIT_FLAG: &str = "--quit";

const PORT: u16 = 47_311;
const SHOW: &[u8; 20] = b"ibasso-control show\n";
const QUIT: &[u8; 20] = b"ibasso-control quit\n";

fn address() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, PORT))
}

/// Sends a request to the running copy. Returns false when no copy is running.
pub fn send(signal: Signal) -> bool {
    let message = match signal {
        Signal::Show => SHOW,
        Signal::Quit => QUIT,
        Signal::Volume(_) => return false,
    };
    match TcpStream::connect_timeout(&address(), Duration::from_millis(500)) {
        Ok(mut stream) => stream.write_all(message).is_ok(),
        Err(_) => false,
    }
}

/// Returns true when this is the only running copy; `on_request` is then called for each request from a
/// later copy. Returns false after asking the running copy to show its window.
pub fn claim(on_request: impl Fn(Signal) + Send + 'static) -> bool {
    match TcpListener::bind(address()) {
        Ok(listener) => {
            let _ = std::thread::Builder::new().name("instance".into()).spawn(move || {
                for mut stream in listener.incoming().flatten() {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                    let mut buffer = [0u8; SHOW.len()];
                    if stream.read_exact(&mut buffer).is_ok() {
                        if &buffer == SHOW {
                            on_request(Signal::Show);
                        } else if &buffer == QUIT {
                            on_request(Signal::Quit);
                        }
                    }
                }
            });
            true
        }
        // The port is taken: most likely by a running copy. If it is something else, just run normally.
        Err(_) => !send(Signal::Show),
    }
}
