fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    let version = env!("CARGO_PKG_VERSION");
    let ver4 = to_file_version(version);
    let manifest_src = match std::fs::read_to_string("assets/app.manifest") {
        Ok(s) => s,
        Err(err) => {
            println!("cargo:warning=could not read app.manifest: {err}");
            return;
        }
    };
    let manifest = patch_app_identity_version(&manifest_src, &ver4);
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_path = out_dir.join("app.manifest");
    if let Err(err) = std::fs::write(&manifest_path, manifest) {
        println!("cargo:warning=could not write generated manifest: {err}");
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set_manifest_file(manifest_path.to_str().unwrap());
    res.set("ProductName", "LuxTray");
    res.set("FileDescription", "Lightweight monitor brightness");
    res.set("LegalCopyright", "Copyright (C) LuxTray");
    res.set("OriginalFilename", "LuxTray.exe");
    res.set("CompanyName", "LuxTray");
    res.set("FileVersion", version);
    res.set("ProductVersion", version);
    if let Err(err) = res.compile() {
        println!("cargo:warning=winresource failed: {err}");
    }
}

fn to_file_version(v: &str) -> String {
    let mut parts: Vec<&str> = v.split('.').collect();
    while parts.len() < 4 {
        parts.push("0");
    }
    parts.truncate(4);
    parts.join(".")
}

fn patch_app_identity_version(src: &str, ver4: &str) -> String {
    let marker = "name=\"LuxTray.App\"";
    let Some(name_at) = src.find(marker) else {
        return src.to_string();
    };
    let prefix = &src[..name_at];
    let Some(ver_key) = prefix.rfind("version=\"") else {
        return src.to_string();
    };
    let ver_val = ver_key + "version=\"".len();
    let Some(end) = src[ver_val..name_at].find('"') else {
        return src.to_string();
    };
    format!("{}{ver4}{}", &src[..ver_val], &src[ver_val + end..])
}
