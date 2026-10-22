//! Pictures are readable only inside the configured content folder.

use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use std::{io::Cursor, path::PathBuf};

#[derive(serde::Deserialize)]
pub(crate) struct Size {
    max_side: Option<u32>,
}

pub(crate) async fn picture(
    State(root): State<PathBuf>,
    Path(path): Path<String>,
    Query(size): Query<Size>,
) -> Result<Response, StatusCode> {
    if size.max_side.is_some_and(|n| n == 0 || n > 8192) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let root = tokio::fs::canonicalize(root)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let path = tokio::fs::canonicalize(root.join(path))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if !path.starts_with(&root) {
        return Err(StatusCode::NOT_FOUND);
    }
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg") {
        return Err(StatusCode::NOT_FOUND);
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let (bytes, mime) = match size.max_side {
        None => (
            bytes,
            if extension == "png" {
                "image/png"
            } else {
                "image/jpeg"
            },
        ),
        Some(side) => {
            let resized = tokio::task::spawn_blocking(move || {
                let picture = image::load_from_memory(&bytes)?;
                let picture = picture.thumbnail(side, side);
                let mut result = Cursor::new(Vec::new());
                picture.write_to(&mut result, image::ImageFormat::Png)?;
                Ok::<_, image::ImageError>(result.into_inner())
            })
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
            (resized, "image/png")
        }
    };
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, max-age=3600"),
        ],
        bytes,
    )
        .into_response())
}
