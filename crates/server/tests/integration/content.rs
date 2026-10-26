//! Picture requests cannot follow a path or a link outside content, and a PDF upload returns a
//! contained staging receipt and respects the body limit.

use std::time::{Duration, SystemTime};

use contract::UploadedPdf;

use super::support::{TestServer, configured};

#[tokio::test]
async fn pictures_and_uploads_stay_inside_their_folders_and_within_their_limits() {
    let server = configured(None, |settings, content| {
        *content = settings.home.join("content");
        std::fs::create_dir_all(&content).unwrap();
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../samples/content/option-volatility-and-pricing/chapter-1/page-num-1/page.png",
        );
        std::fs::copy(sample, content.join("page.png")).unwrap();
        std::fs::copy(content.join("page.png"), settings.home.join("outside.png")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(settings.home.join("outside.png"), content.join("link.png"))
            .unwrap();
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

    pictures_are_contained_and_resize_to_a_png_with_cache_headers(&server).await;
    uploads_validate_and_stage_pdfs_with_limits_and_old_folder_cleanup(&server).await;
}

async fn pictures_are_contained_and_resize_to_a_png_with_cache_headers(server: &TestServer) {
    let client = server.client();
    let content = server.home.path().join("content");
    let reference = rag_core::content_url(&content.join("page.png"), &content).unwrap();
    assert_eq!(reference, "content/page.png");
    assert!(rag_core::content_url(&server.home.path().join("outside.png"), &content).is_none());
    assert!(rag_core::content_url(&content.join("../outside.png"), &content).is_none());
    let response = client
        .get(server.url(&format!("/{reference}")))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "private, max-age=3600");
    let original = response.bytes().await.unwrap();
    assert_eq!(
        original.as_ref(),
        std::fs::read(server.home.path().join("content/page.png")).unwrap()
    );
    for path in [
        "/content/%2E%2E%2Foutside.png",
        "/content/%2Foutside.png",
        "/content/link.png",
    ] {
        assert!(
            !client
                .get(server.url(path))
                .send()
                .await
                .unwrap()
                .status()
                .is_success(),
            "{path}"
        );
    }
    let response = client
        .get(server.url("/content/page.png?max_side=200"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "image/png");
    let image = image::load_from_memory(&response.bytes().await.unwrap()).unwrap();
    assert!(image.width() <= 200 && image.height() <= 200);
    assert_eq!(
        client
            .get(server.url("/content/page.png?max_side=8193"))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
}

async fn uploads_validate_and_stage_pdfs_with_limits_and_old_folder_cleanup(server: &TestServer) {
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
