//! Exports are claimed once before their bytes leave the server.

use crate::{Settings, content::contained};
use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

pub(crate) async fn download(
    State(settings): State<Settings>,
    Path((id, file)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    let id = uuid::Uuid::parse_str(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    if file.is_empty()
        || file == "."
        || file == ".."
        || file.contains(['/', '\\'])
        || file.chars().any(char::is_control)
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let root = settings.home.join(backend::EXPORTS);
    let folder = root.join(id.to_string());
    let path = contained(&root, format!("{id}/{file}")).await?;
    let claimed = folder.join(format!(".download-{}", uuid::Uuid::new_v4()));
    tokio::fs::rename(&path, &claimed)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let mut body = ExportBody {
        bytes: None,
        claimed,
        folder,
    };
    body.bytes = Some(
        tokio::fs::read(&body.claimed)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    let ascii: String = file
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || " ._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let disposition = format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{}",
        utf8_percent_encode(&file, NON_ALPHANUMERIC)
    );
    Ok((
        [
            (header::CONTENT_DISPOSITION, disposition),
            (header::CONTENT_TYPE, crate::mime(&file).into()),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        axum::body::Body::from_stream(futures_util::stream::unfold(body, |mut body| async move {
            body.bytes
                .take()
                .map(|bytes| (Ok::<_, std::io::Error>(bytes), body))
        })),
    )
        .into_response())
}

struct ExportBody {
    bytes: Option<Vec<u8>>,
    claimed: std::path::PathBuf,
    folder: std::path::PathBuf,
}

impl Drop for ExportBody {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.claimed) {
            tracing::warn!(%error, "could not remove the consumed export");
        }
        if let Err(error) = std::fs::remove_dir(&self.folder)
            && error.kind() != std::io::ErrorKind::DirectoryNotEmpty
        {
            tracing::debug!(%error, "could not remove the export folder");
        }
    }
}
