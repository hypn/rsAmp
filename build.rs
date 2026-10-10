//! Embeds the application icon and version info into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/rsamp.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/rsamp.ico")
            .set("ProductName", "rsAmp")
            .set("FileDescription", "rsAmp")
            .set("InternalName", "rsAmp.exe")
            .set("OriginalFilename", "rsAmp.exe");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
