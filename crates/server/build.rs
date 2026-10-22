//! The server can start before the browser bundle has been built.

fn main() {
    println!("cargo:rustc-check-cfg=cfg(ui_bundle)");
    println!("cargo:rerun-if-changed=../web-ui/dist");
    println!(
        "cargo:rustc-env=QUANTY_UI_BUILT={}",
        std::path::Path::new("../web-ui/dist/index.html").is_file()
    );
    if std::path::Path::new("../web-ui/dist").is_dir() {
        println!("cargo:rustc-cfg=ui_bundle");
    }
}
