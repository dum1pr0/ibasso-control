//! Read-only diagnostic: lists HID collections of iBasso devices and dumps their report descriptors.
//! It never sends a command to the device.
use hidapi::{HidApi, MAX_REPORT_DESCRIPTOR_SIZE};
use ibasso_control::ibasso::VENDOR_IDS;

fn main() {
    let api = HidApi::new().expect("hidapi");
    for info in api.device_list().filter(|d| VENDOR_IDS.contains(&d.vendor_id())) {
        println!(
            "{:04x}:{:04x} product={:?} interface={} usage_page={:#06x} usage={:#06x}\n  path={}",
            info.vendor_id(),
            info.product_id(),
            info.product_string(),
            info.interface_number(),
            info.usage_page(),
            info.usage(),
            info.path().to_string_lossy()
        );
        match info.open_device(&api) {
            Ok(device) => {
                let mut buffer = [0u8; MAX_REPORT_DESCRIPTOR_SIZE];
                match device.get_report_descriptor(&mut buffer) {
                    Ok(n) => {
                        let hex: Vec<String> = buffer[..n].iter().map(|b| format!("{b:02x}")).collect();
                        println!("  descriptor[{n}]={}", hex.join(" "));
                    }
                    Err(e) => println!("  descriptor error: {e}"),
                }
            }
            Err(e) => println!("  open error: {e}"),
        }
    }
}
