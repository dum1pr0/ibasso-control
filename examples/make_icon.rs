//! Regenerates assets/icon.ico (embedded into the Windows .exe) and assets/icon.png (for Linux desktop entries)
//! from the icon drawn in src/icon.rs.
use ibasso_control::icon;

fn main() {
    std::fs::create_dir_all("assets").expect("create assets");
    std::fs::write("assets/icon.ico", icon::ico(&[16, 20, 24, 32, 40, 48, 64, 256])).expect("write ico");
    std::fs::write("assets/icon.png", icon::png(256)).expect("write png");
    println!("wrote assets/icon.ico and assets/icon.png");
}
