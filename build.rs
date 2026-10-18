fn main() {
    println!("cargo:rerun-if-changed=packaging/windows/app-icon.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("packaging/windows/app-icon.ico")
            .compile()
            .expect("Could not compile the Windows app icon and product information");
    }
}
