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
        ehttp::fetch(request, move |result| {
            let uploaded = match result {
                Ok(response) if response.status == 200 => serde_json::from_slice(&response.bytes)
                    .map_err(|error| {
                        format!("Could not read the upload response: {error}. Please try again.")
                    }),
                Ok(response) => Err(match response.status {
                    413 => "This PDF is too large to upload.".to_owned(),
                    415 => "The selected file is not a PDF.".to_owned(),
                    401 | 403 => "Please sign in again, then upload the PDF.".to_owned(),
                    status => format!(
                        "The server could not save the PDF (HTTP {status}). Please try again or contact the server administrator."
                    ),
                }),
                Err(error) => Err(format!(
                    "Could not upload the PDF: {error}. Please try again."
                )),
            };
            let intent = match uploaded {
                Ok(pdf) => Intent::PdfPicked(pdf),
                Err(error) => {
                    log::error!("PDF upload failed: {error}");
                    Intent::PdfUploadFailed(error)
                }
            };
            if sender.send(intent).is_ok() {
                ctx.request_repaint();
            }
        });
    });
}
