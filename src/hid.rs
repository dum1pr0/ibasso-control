//! USB HID discovery and the per-device worker thread.
use crate::ibasso::{Ctl, Family, Frame, PeqBand, Session, Via, VENDOR_IDS};
use hidapi::{HidApi, HidDevice};
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbEntry {
    pub path: CString,
    pub vid: u16,
    pub pid: u16,
    pub product: String,
    /// None when the product string is not one of the models the iBasso app controls.
    pub family: Option<Family>,
}

/// Lists attached iBasso devices, one entry per physical device.
pub fn scan(api: &mut HidApi) -> Vec<UsbEntry> {
    let _ = api.refresh_devices();
    let mut out: Vec<UsbEntry> = Vec::new();
    for info in api.device_list().filter(|d| VENDOR_IDS.contains(&d.vendor_id())) {
        let product = info.product_string().unwrap_or("").trim().to_string();
        let (vid, pid) = (info.vendor_id(), info.product_id());
        let family = Family::from_product(&product);
        // The iBasso app talks to the plain HID interface; prefer a vendor usage page when there are several.
        match out.iter_mut().find(|e| e.vid == vid && e.pid == pid && e.product == product) {
            Some(existing) if info.usage_page() >= 0xFF00 => existing.path = info.path().to_owned(),
            Some(_) => {}
            None => out.push(UsbEntry { path: info.path().to_owned(), vid, pid, product, family }),
        }
    }
    out
}

pub enum Command {
    Set(Ctl, i32),
    WritePeq { slot: u8, bands: Box<[PeqBand; 10]>, total_gain: i8 },
    Close,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub values: HashMap<Ctl, i32>,
    pub firmware: Option<String>,
    pub peq: Option<[[PeqBand; 10]; 3]>,
    pub peq_gain: [i8; 3],
    pub error: Option<String>,
    pub closed: bool,
    /// Connect queries answered so far and in total, while the initial read is still running.
    pub progress: Option<(usize, usize)>,
    /// True once the initial read has finished and the values reflect the device.
    pub ready: bool,
}

pub struct Handle {
    pub tx: Sender<Command>,
    pub rx: Receiver<Snapshot>,
}

/// True when a HID report descriptor declares numbered reports (a Report ID item).
pub fn uses_report_ids(descriptor: &[u8]) -> bool {
    let mut index = 0;
    while let Some(&prefix) = descriptor.get(index) {
        if prefix == 0xFE {
            // Long item: prefix, data size, tag, data.
            index += 3 + *descriptor.get(index + 1).unwrap_or(&0) as usize;
            continue;
        }
        if prefix & 0xFC == 0x84 {
            return true;
        }
        index += 1 + [0, 1, 2, 4][(prefix & 0x03) as usize];
    }
    false
}

/// Bytes handed to hidapi, whose first byte is always the report ID.
/// With numbered reports the vendor frame already starts with its report ID, such as 0x4B on the DC04 Ultra.
pub fn outgoing(frame: &Frame, numbered: bool) -> Vec<u8> {
    if numbered && frame.via == Via::Interrupt {
        return frame.bytes.clone();
    }
    let mut buffer = Vec::with_capacity(frame.bytes.len() + 1);
    buffer.push(0u8);
    buffer.extend_from_slice(&frame.bytes);
    buffer
}

/// Reads the device's report descriptor to learn how its reports are addressed.
pub fn device_uses_report_ids(device: &HidDevice) -> bool {
    let mut descriptor = [0u8; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
    device.get_report_descriptor(&mut descriptor).map(|n| uses_report_ids(&descriptor[..n])).unwrap_or(false)
}

pub fn send(device: &HidDevice, frame: &Frame, numbered: bool) -> Result<(), String> {
    let buffer = outgoing(frame, numbered);
    let result = match frame.via {
        Via::Interrupt => device.write(&buffer).map(|_| ()),
        Via::Control => device.send_output_report(&buffer).or_else(|_| device.write(&buffer).map(|_| ())),
    };
    std::thread::sleep(Duration::from_millis(30));
    result.map_err(|e| format!("Lỗi gửi lệnh: {e}"))
}

/// Sends a status query and waits for its reply. Like the vendor app, it waits 200 ms and retries once,
/// because the device can drop a query that arrives while it is busy.
pub fn query(device: &HidDevice, session: &mut Session, frame: &Frame, numbered: bool) -> Result<bool, String> {
    let mut buffer = [0u8; 65];
    for _ in 0..2 {
        send(device, frame, numbered)?;
        let deadline = Instant::now() + Duration::from_millis(200);
        while Instant::now() < deadline {
            let n = device.read_timeout(&mut buffer, 20).map_err(|e| format!("Mất kết nối: {e}"))?;
            if n > 0 {
                session.on_report(&buffer[..n]);
                if session.answers(frame, &buffer[..n]) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Sends a setting write. Writes the device acknowledges are repeated until it does, because it drops some
/// of them; without this the app shows the new value while the device keeps the old one.
pub fn write(device: &HidDevice, session: &mut Session, frame: &Frame, numbered: bool) -> Result<(), String> {
    if !session.confirms(frame) {
        return send(device, frame, numbered);
    }
    let mut buffer = [0u8; 65];
    for _ in 0..4 {
        send(device, frame, numbered)?;
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            let n = device.read_timeout(&mut buffer, 20).map_err(|e| format!("Mất kết nối: {e}"))?;
            if n > 0 {
                session.on_report(&buffer[..n]);
                if session.acknowledges(frame, &buffer[..n]) {
                    return Ok(());
                }
            }
        }
    }
    // Show what the device really has instead of the value that was not applied.
    if let Some(read) = session.read_back(frame) {
        query(device, session, &read, numbered)?;
    }
    Err("Thiết bị không nhận lệnh sau 4 lần gửi, hãy thử lại.".into())
}

/// Writes one PEQ slot. The device does not acknowledge band writes, so the bands are read back and any that
/// differ are written again before the slot is saved and activated.
pub fn write_peq(
    device: &HidDevice,
    session: &mut Session,
    slot: u8,
    bands: &[PeqBand; 10],
    total_gain: i8,
    numbered: bool,
) -> Result<(), String> {
    let frames = crate::ibasso::peq_write_frames(slot, bands, total_gain);
    let (band_frames, finish) = frames.split_at(10);
    let reads = crate::ibasso::peq_read_frames(slot);
    let mut pending: Vec<usize> = (0..10).collect();
    for _ in 0..4 {
        for &band in &pending {
            send(device, &band_frames[band], numbered)?;
        }
        pending.retain(|&band| {
            let read = query(device, session, &reads[band], numbered).unwrap_or(false);
            let stored = &session.peq[slot as usize][band];
            !(read && stored.stored() == bands[band].stored() && session.peq_gain[slot as usize] == total_gain)
        });
        if pending.is_empty() {
            break;
        }
    }
    session.peq[slot as usize] = *bands;
    session.peq_gain[slot as usize] = total_gain;
    if !pending.is_empty() {
        let list: Vec<String> = pending.iter().map(|b| (b + 1).to_string()).collect();
        return Err(format!("Thiết bị không nhận dải {} của EQ, chưa lưu. Hãy ghi lại.", list.join(", ")));
    }
    // Save and activate are not acknowledged either. Both are idempotent, so sending them twice makes a
    // dropped one unlikely.
    for frame in finish.iter().chain(finish) {
        send(device, frame, numbered)?;
    }
    Ok(())
}

/// Opens the device and runs its protocol session on a dedicated thread.
pub fn open(api: &HidApi, entry: &UsbEntry, family: Family) -> Result<Handle, String> {
    let device = api.open_path(&entry.path).map_err(|e| format!("Không mở được thiết bị: {e}"))?;
    let (cmd_tx, cmd_rx) = channel::<Command>();
    let (snap_tx, snap_rx) = channel::<Snapshot>();
    std::thread::spawn(move || worker(device, family, cmd_rx, snap_tx));
    Ok(Handle { tx: cmd_tx, rx: snap_rx })
}

fn worker(device: HidDevice, family: Family, commands: Receiver<Command>, snapshots: Sender<Snapshot>) {
    let mut session = Session::new(family);
    let mut error = None;
    let numbered = device_uses_report_ids(&device);
    let mut buffer = [0u8; 65];
    let frames = session.init_frames();
    let total = frames.len();
    for (index, frame) in frames.iter().enumerate() {
        let progress = Snapshot { progress: Some((index, total)), ..Default::default() };
        if snapshots.send(progress).is_err() {
            return;
        }
        if let Err(e) = query(&device, &mut session, frame, numbered) {
            error = Some(e);
            break;
        }
    }
    // The connect queries already consumed their replies, so publish that state once up front.
    let mut publish_initial_state = true;
    loop {
        let mut changed = std::mem::take(&mut publish_initial_state) || error.is_some();
        while let Ok(command) = commands.try_recv() {
            changed = true;
            let result = match command {
                Command::Close => {
                    let _ = snapshots.send(Snapshot { closed: true, ..Default::default() });
                    return;
                }
                Command::Set(ctl, value) => session.set(ctl, value).and_then(|frames| {
                    frames.iter().try_for_each(|frame| write(&device, &mut session, frame, numbered))
                }),
                Command::WritePeq { slot, bands, total_gain } => {
                    write_peq(&device, &mut session, slot, &bands, total_gain, numbered)
                }
            };
            if let Err(e) = result {
                error = Some(e);
            }
        }
        match device.read_timeout(&mut buffer, 20) {
            Ok(0) => {}
            Ok(n) => {
                changed = true;
                for frame in session.on_report(&buffer[..n]) {
                    if let Err(e) = send(&device, &frame, numbered) {
                        error = Some(e);
                    }
                }
            }
            Err(e) => {
                let _ = snapshots.send(Snapshot {
                    error: Some(format!("Mất kết nối: {e}")),
                    closed: true,
                    ..Default::default()
                });
                return;
            }
        }
        if changed {
            let snapshot = Snapshot {
                values: session.values.clone(),
                firmware: session.firmware.clone(),
                peq: (family.has_peq() && session.peq_ready()).then_some(session.peq),
                peq_gain: session.peq_gain,
                error: error.take(),
                closed: false,
                progress: None,
                ready: true,
            };
            if snapshots.send(snapshot).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Report descriptor read from a real DC04 Ultra (0661:0883).
    const DC04U: &[u8] = &[
        0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x85, 0x03, 0x09, 0xe9, 0x09, 0xea, 0x09, 0xcd, 0x09, 0xcf, 0x09, 0xb6,
        0x09, 0xb5, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x06, 0x81, 0x02, 0x75, 0x02, 0x95, 0x01, 0x81, 0x03,
        0x85, 0x4b, 0x06, 0x01, 0xff, 0x09, 0x01, 0x15, 0x00, 0x25, 0x03, 0x75, 0x08, 0x95, 0x3f, 0x81, 0x03, 0x85,
        0x01, 0x09, 0x03, 0x15, 0x00, 0x25, 0x03, 0x75, 0x08, 0x95, 0x3f, 0x81, 0x03, 0x85, 0x4b, 0x09, 0x02, 0x15,
        0x00, 0x25, 0x03, 0x75, 0x08, 0x95, 0x3f, 0x91, 0x02, 0x85, 0x01, 0x09, 0x04, 0x15, 0x00, 0x25, 0x03, 0x75,
        0x08, 0x95, 0x3f, 0x91, 0x02, 0xc0,
    ];

    #[test]
    fn detects_numbered_reports() {
        assert!(uses_report_ids(DC04U));
        assert!(!uses_report_ids(&[0x06, 0x00, 0xff, 0x09, 0x01, 0xa1, 0x01, 0x95, 0x40, 0x91, 0x02, 0xc0]));
    }

    #[test]
    fn a_data_byte_equal_to_the_report_id_tag_is_not_mistaken_for_one() {
        assert!(!uses_report_ids(&[0x09, 0x85, 0x15, 0x84, 0xc0]));
    }

    #[test]
    fn numbered_interrupt_frames_keep_their_own_report_id() {
        let frame = Frame { via: Via::Interrupt, bytes: vec![75; 64] };
        assert_eq!(outgoing(&frame, true).len(), 64);
        assert_eq!(outgoing(&frame, false).len(), 65);
    }

    #[test]
    fn control_frames_always_use_report_zero() {
        let frame = Frame { via: Via::Control, bytes: vec![1; 16] };
        assert_eq!(outgoing(&frame, true)[0], 0);
    }
}
