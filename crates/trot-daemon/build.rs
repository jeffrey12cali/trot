// On macOS, embed an Info.plist into the binary's __TEXT,__info_plist section so
// the daemon carries its own Bluetooth usage description. Without this, macOS
// kills the process the moment it touches CoreBluetooth ("attempted to access
// privacy-sensitive data without a usage description").
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let plist = format!("{manifest}/Info.plist");
        println!("cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{plist}");
        println!("cargo:rerun-if-changed=Info.plist");
    }
}
