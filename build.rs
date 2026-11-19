//! Embeds the app icon into the Windows executable so Explorer and the taskbar show it.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icon.ico");
    resource.set("FileDescription", "iBasso Control");
    resource.set("ProductName", "iBasso Control");
    if let Err(e) = resource.compile() {
        // The app still builds and shows its window icon; only the .exe file icon is missing.
        println!("cargo:warning=could not embed the icon resource: {e}");
    }
}
