//! Diagnostic: changes the DC04 Ultra screen timeout, reads it back, then restores it.
//! The screen timeout does not affect audio, so it is a harmless way to confirm writes.
use hidapi::HidApi;
use ibasso_control::hid::{self, device_uses_report_ids, query, send};
use ibasso_control::ibasso::{Ctl, Family, Session};

fn main() {
    let mut api = HidApi::new().expect("hidapi");
    let entry = hid::scan(&mut api).into_iter().find(|e| e.family == Some(Family::Dc04u)).expect("no DC04 Ultra");
    let device = api.open_path(&entry.path).expect("open");
    let numbered = device_uses_report_ids(&device);
    let mut session = Session::new(Family::Dc04u);
    let reads = session.init_frames();
    for frame in &reads {
        query(&device, &mut session, frame, numbered).expect("query");
    }
    let original = *session.values.get(&Ctl::ScreenTime).expect("screen time not reported");
    let target = if original == 2 { 1 } else { 2 };
    let read_back = |session: &mut Session| {
        // Re-read the screen group (the fifth connect query).
        query(&device, session, &reads[4], numbered).expect("query");
        session.values.get(&Ctl::ScreenTime).copied()
    };
    for frame in session.set(Ctl::ScreenTime, target).expect("set") {
        send(&device, &frame, numbered).expect("send");
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    let changed = read_back(&mut session);
    for frame in session.set(Ctl::ScreenTime, original).expect("restore") {
        send(&device, &frame, numbered).expect("send");
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    let restored = read_back(&mut session);
    println!("screen time {original} -> {target}: device reported {changed:?}; restored to {restored:?}");
    let mut values: Vec<_> = session.values.iter().map(|(k, v)| format!("{k:?}={v}")).collect();
    values.sort();
    println!("{}", values.join(" "));
}
