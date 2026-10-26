//! Staged PDFs live outside the content library until an ingest accepts them.

use crate::Settings;
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use contract::{UploadId, UploadedPdf};
use percent_encoding::percent_decode_str;
use std::path::Path;
use std::time::{Duration, SystemTime};

pub(crate) async fn upload(
    State(settings): State<Settings>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadedPdf>, StatusCode> {
    if !body.starts_with(b"%PDF-") {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let name = headers
        .get("x-file-name")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let decoded = percent_decode_str(name)
        .decode_utf8()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let file_name: String = decoded
        .chars()
        .filter(|c| *c != '/' && *c != '\\' && !c.is_control())
        .collect();
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return Err(StatusCode::BAD_REQUEST);
    }
    let upload = UploadId(uuid::Uuid::new_v4());
    let folder = settings
        .home
        .join(backend::UPLOADS)
        .join(upload.0.to_string());
    tokio::fs::create_dir_all(&folder).await.map_err(|error| {
        tracing::error!(%error, "could not create the PDF staging folder");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    tokio::fs::write(folder.join(&file_name), &body)
        .await
        .map_err(|error| {
            tracing::error!(%error, "could not stage the uploaded PDF");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(UploadedPdf {
        upload,
        file_name,
        bytes: body.len() as u64,
    }))
}

pub(crate) fn clean_old_folders(root: &Path, age: Duration) -> std::io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if SystemTime::now()
            .duration_since(metadata.modified()?)
            .is_ok_and(|elapsed| elapsed > age)
        {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}
