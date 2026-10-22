//! A PDF upload returns a contained staging receipt and respects the body limit.

use super::support::configured;
use contract::UploadedPdf;
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn uploads_validate_and_stage_pdfs_with_limits_and_old_folder_cleanup() {
    let server = configured(None, |settings, _| {
        settings.upload_limit = 1024 * 1024;
        for (name, days) in [("old", 8), ("recent", 1)] {
            let folder = settings.home.join("data/uploads").join(name);
            std::fs::create_dir_all(&folder).unwrap();
            let times = std::fs::FileTimes::new()
                .set_modified(SystemTime::now() - Duration::from_secs(days * 86400));
            std::fs::File::open(folder)
                .unwrap()
                .set_times(times)
                .unwrap();
        }
    })
    .await;
    assert!(!server.home.path().join("data/uploads/old").exists());
    assert!(server.home.path().join("data/uploads/recent").exists());
    let client = server.client();
    let pdf = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/chapter-1-sample-pages.pdf"),
    )
    .unwrap();
    let response = client
        .post(server.url("/api/upload"))
        .header("x-file-name", "..%2F%5Csample.pdf")
        .body(pdf.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let receipt: UploadedPdf = response.json().await.unwrap();
    assert_eq!(receipt.file_name, "..sample.pdf");
    assert_eq!(receipt.bytes, pdf.len() as u64);
    let staged = server
        .home
        .path()
        .join("data/uploads")
        .join(receipt.upload.0.to_string())
        .join(&receipt.file_name);
    assert_eq!(std::fs::read(staged).unwrap(), pdf);
    assert_eq!(
        client
            .post(server.url("/api/upload"))
            .header("x-file-name", "bad.pdf")
            .body("this is not a PDF")
            .send()
            .await
            .unwrap()
            .status(),
        415
    );
    assert_eq!(
        client
            .post(server.url("/api/upload"))
            .header("x-file-name", "large.pdf")
            .body(vec![0; 1024 * 1024 + 1])
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
}
