//! Windows: Icon in die .exe einbetten (Explorer, Taskleiste). Andere Ziele: nichts zu tun.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("Icon für Windows einbetten");
    }
}
