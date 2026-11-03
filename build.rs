fn main() {
    println!("cargo:rerun-if-env-changed=ENDERCORD_INSTALLER_HASH");
    println!("cargo:rerun-if-env-changed=ENDERCORD_INSTALLER_TAG");

    // Resources need rc.exe, so they're only embedded when building on Windows for Windows.
    if cfg!(windows) && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("winres/icon.ico");
        res.set("ProductName", "EnderCord Installer");
        res.set("FileDescription", "EnderCord Installer");
        res.compile().expect("Failed to embed Windows resources");
    }
}
