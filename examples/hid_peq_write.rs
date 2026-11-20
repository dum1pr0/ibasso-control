//! Diagnostic: prints a PEQ slot, writes an Equalizer APO / squig.link file into it through the app's
//! verified write path, then reads the slot back from the device.
//! Usage: hid_peq_write <slot 1-3> [eq.txt]   (without a file it only prints the slot)
use hidapi::HidApi;
use ibasso_control::hid::{self, device_uses_report_ids, query};
use ibasso_control::ibasso::{peq_read_frames, Family, Session};

fn print_slot(session: &Session, slot: usize) {
    println!("  total gain {} dB", session.peq_gain[slot]);
    for (i, b) in session.peq[slot].iter().enumerate() {
        println!("  band {:>2}: type {} {:>5} Hz gain {:>6.2} dB Q {:.3}", i + 1, b.filter_type, b.frequency, b.gain, b.q);
    }
}

fn read_slot(device: &hidapi::HidDevice, session: &mut Session, slot: u8, numbered: bool) {
    for frame in peq_read_frames(slot) {
        assert!(query(device, session, &frame, numbered).expect("query"), "no reply");
    }
}

fn main() {
    let slot: u8 = std::env::args().nth(1).and_then(|a| a.parse::<u8>().ok()).filter(|s| (1..=3).contains(s)).expect("slot 1-3") - 1;
    let mut api = HidApi::new().expect("hidapi");
    let entry = hid::scan(&mut api).into_iter().find(|e| e.family == Some(Family::Dc04u)).expect("no DC04 Ultra");
    let device = api.open_path(&entry.path).expect("open");
    let numbered = device_uses_report_ids(&device);
    let mut session = Session::new(Family::Dc04u);
    read_slot(&device, &mut session, slot, numbered);
    println!("Slot {} before:", slot + 1);
    print_slot(&session, slot as usize);
    let Some(path) = std::env::args().nth(2) else { return };
    let eq = ibasso_control::peq_import::parse(&std::fs::read_to_string(path).expect("read file")).expect("parse");
    for note in &eq.notes {
        println!("note: {note}");
    }
    match hid::write_peq(&device, &mut session, slot, &eq.bands, eq.total_gain, numbered) {
        Ok(()) => println!("write: ok"),
        Err(e) => println!("write: {e}"),
    }
    let mut fresh = Session::new(Family::Dc04u);
    read_slot(&device, &mut fresh, slot, numbered);
    println!("Slot {} after (read from device):", slot + 1);
    print_slot(&fresh, slot as usize);
    let same = fresh.peq[slot as usize].iter().zip(&eq.bands).all(|(a, b)| a.stored() == b.stored())
        && fresh.peq_gain[slot as usize] == eq.total_gain;
    println!("matches file: {same}");
}
