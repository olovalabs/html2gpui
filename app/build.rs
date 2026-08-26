fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/logo/olova.ico");
        res.set("ProductName", "Olova Editor");
        res.set("FileDescription", "A native code editor built with GPUI");
        if let Err(e) = res.compile() {
            // Fallback: try setting manifest manually
            eprintln!("winres compile failed: {e}");
        }
    }

    // Re-run if the icon changes
    println!("cargo:rerun-if-changed=assets/logo/olova.ico");
    println!("cargo:rerun-if-changed=build.rs");
}
