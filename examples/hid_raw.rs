//! Diagnostic: sends the read-only connect queries to each attached iBasso device and prints raw replies.
use hidapi::HidApi;
use ibasso_control::hid::{self, device_uses_report_ids, outgoing};
use ibasso_control::ibasso::Session;
use std::time::{Duration, Instant};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    let mut api = HidApi::new().expect("hidapi");
    for entry in hid::scan(&mut api) {
        let Some(family) = entry.family else { continue };
        println!("== {} ({family:?})", entry.product);
        let device = api.open_path(&entry.path).expect("open");
        let numbered = device_uses_report_ids(&device);
        let session = Session::new(family);
        // Skip the 30 equalizer reads to keep the output short.
        for frame in session.init_frames().into_iter().take(6) {
            let bytes = outgoing(&frame, numbered);
            println!("tx {:?} {}", frame.via, hex(&bytes));
            let sent = match frame.via {
                ibasso_control::ibasso::Via::Interrupt => device.write(&bytes).map(|_| ()),
                ibasso_control::ibasso::Via::Control => device.send_output_report(&bytes),
            };
            if let Err(e) = sent {
                println!("   send error: {e}");
                continue;
            }
            let mut buffer = [0u8; 65];
            let deadline = Instant::now() + Duration::from_millis(400);
            while Instant::now() < deadline {
                if let Ok(n) = device.read_timeout(&mut buffer, 50) {
                    if n > 0 {
                        println!("   rx {}", hex(&buffer[..n.min(24)]));
                    }
                }
            }
        }
    }
}
