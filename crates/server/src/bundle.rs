//! The executable carries a versioned browser bundle so deployment needs one binary.

use axum::{
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

#[cfg(ui_bundle)]
#[derive(rust_embed::Embed)]
#[folder = "../web-ui/dist"]
struct Bundle;

pub(crate) async fn serve(request: Request) -> Response {
    let path = request.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    #[cfg(ui_bundle)]
    if let Some(file) = Bundle::get(path) {
        let cache = if path == "index.html" {
            "no-store"
        } else {
            "public, max-age=31536000, immutable"
        };
        return (
            [
                (header::CONTENT_TYPE, crate::mime(path)),
                (header::CACHE_CONTROL, cache),
            ],
            file.data.into_owned(),
        )
            .into_response();
    }
    if path == "index.html" {
        return (StatusCode::SERVICE_UNAVAILABLE, [(header::CACHE_CONTROL, "no-store")], "The UI bundle is not built. Run `trunk build --release` in crates/web-ui, then build quanty again.").into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}
