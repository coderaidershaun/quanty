//! Picture requests cannot follow a path or a link outside content.

use super::support::configured;

#[tokio::test]
async fn pictures_are_contained_and_resize_to_a_png_with_cache_headers() {
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
    })
    .await;
    let client = server.client();
    let response = client
        .get(server.url("/content/page.png"))
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
