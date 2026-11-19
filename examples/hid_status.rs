//! Diagnostic: for every attached iBasso device, sends only the status queries the app sends on connect,
//! then prints what the device reports. It never changes a setting.
use hidapi::HidApi;
use ibasso_control::hid::{self, device_uses_report_ids, query};
use ibasso_control::ibasso::Session;

fn main() {
    let mut api = HidApi::new().expect("hidapi");
    let entries = hid::scan(&mut api);
    if entries.is_empty() {
        println!("no iBasso device attached");
    }
    for entry in entries {
        println!("== {} ({:04x}:{:04x})", entry.product, entry.vid, entry.pid);
        let Some(family) = entry.family else {
            println!("   not a supported model");
            continue;
        };
        let device = match api.open_path(&entry.path) {
            Ok(device) => device,
            Err(e) => {
                println!("   open failed: {e}");
                continue;
            }
        };
        let numbered = device_uses_report_ids(&device);
        let mut session = Session::new(family);
        let frames = session.init_frames();
        let mut answered = 0;
        for frame in &frames {
            match query(&device, &mut session, frame, numbered) {
                Ok(true) => answered += 1,
                Ok(false) => {}
                Err(e) => println!("   query error: {e}"),
            }
        }
        let mut values: Vec<_> = session.values.iter().map(|(k, v)| format!("{k:?}={v}")).collect();
        values.sort();
        println!(
            "   {family:?} numbered_reports={numbered} answered={answered}/{} firmware={:?} peq_ready={}",
            frames.len(),
            session.firmware,
            session.peq_ready()
        );
        println!("   {}", values.join(" "));
    }
}
