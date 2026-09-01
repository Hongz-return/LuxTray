fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");

    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set_manifest_file("assets/app.manifest");
    res.set("ProductName", "LuxTray");
    res.set("FileDescription", "Lightweight monitor brightness");
    res.set("LegalCopyright", "Copyright (C) LuxTray");
    res.set("OriginalFilename", "LuxTray.exe");
    res.set("CompanyName", "LuxTray");
    if let Err(err) = res.compile() {
        println!("cargo:warning=winresource failed: {err}");
    }
}
