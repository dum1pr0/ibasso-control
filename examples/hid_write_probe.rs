//! Diagnostic: toggles the DC04 Ultra screen timeout many times through the app's confirmed write path and
//! reads the setting back after each write. The screen timeout does not affect audio. The device silently
//! drops some writes, so without confirmation a few rounds would end with the old value.
use hidapi::HidApi;
use ibasso_control::hid::{self, device_uses_report_ids, query};
use ibasso_control::ibasso::{Ctl, Family, Session};

fn main() {
    let mut api = HidApi::new().expect("hidapi");
    let entry = hid::scan(&mut api).into_iter().find(|e| e.family == Some(Family::Dc04u)).expect("no DC04 Ultra");
    let device = api.open_path(&entry.path).expect("open");
    let numbered = device_uses_report_ids(&device);
    let mut session = Session::new(Family::Dc04u);
    let reads = session.init_frames();
    for frame in reads.iter().take(6) {
        query(&device, &mut session, frame, numbered).expect("query");
    }
    let original = *session.values.get(&Ctl::ScreenTime).expect("screen time");
    let other = if original == 2 { 1 } else { 2 };
    let rounds: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(20);
    let (mut failed, mut wrong) = (0, 0);
    for round in 0..rounds {
        let target = if round % 2 == 0 { other } else { original };
        for frame in session.set(Ctl::ScreenTime, target).unwrap() {
            if let Err(e) = hid::write(&device, &mut session, &frame, numbered) {
                println!("round {round}: {e}");
                failed += 1;
            }
        }
        query(&device, &mut session, &reads[4], numbered).expect("query");
        if session.values.get(&Ctl::ScreenTime) != Some(&target) {
            wrong += 1;
        }
    }
    for frame in session.set(Ctl::ScreenTime, original).unwrap() {
        hid::write(&device, &mut session, &frame, numbered).expect("restore");
    }
    println!("{rounds} writes: {failed} reported failures, {wrong} not applied; screen time restored to {original}");
}
