//! The browser owns the window while the server owns the library and its work.

#[cfg(any(target_arch = "wasm32", test))]
mod back;
#[cfg(target_arch = "wasm32")]
mod fetch;
#[cfg(target_arch = "wasm32")]
mod scheme;
#[cfg(target_arch = "wasm32")]
mod transport;
#[cfg(target_arch = "wasm32")]
mod upload;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("the UI runs in the browser: run `quanty`");
    std::process::exit(2);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    if let Err(error) = eframe::WebLogger::init(log::LevelFilter::Info) {
        web_sys::console::error_1(&format!("failed to install browser logger: {error}").into());
    }
    web::start();
}
