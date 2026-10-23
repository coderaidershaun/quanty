//! Picking a PDF finishes asynchronously, so completion returns through an owned queue.

use std::sync::mpsc::Sender;

use contract::Intent;
use eframe::egui;

pub(crate) fn pick(sender: Sender<Intent>, ctx: egui::Context) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter("PDF", &["pdf"])
            .pick_file()
            .await
        else {
            return;
        };
        let name = percent_encoding::utf8_percent_encode(
            &file.file_name(),
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();
        let mut request = ehttp::Request::post("api/upload", file.read().await);
        request.mode = ehttp::Mode::SameOrigin;
        request.headers =
            ehttp::Headers::new(&[("X-File-Name", &name), ("Content-Type", "application/pdf")]);
        ehttp::fetch(request, move |result| match result {
            Ok(response) if response.status == 200 => {
                match serde_json::from_slice(&response.bytes) {
                    Ok(pdf) => {
                        if sender.send(Intent::PdfPicked(pdf)).is_ok() {
                            ctx.request_repaint();
                        }
                    }
                    Err(error) => log::error!("could not read PDF upload response: {error}"),
                }
            }
            Ok(response) => log::error!("PDF upload failed: HTTP {}", response.status),
            Err(error) => log::error!("PDF upload failed: {error}"),
        });
    });
}
