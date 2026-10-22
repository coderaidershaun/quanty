//! Exports become downloadable files, with pictures resolved inside the content folder.
use super::Reply;
use contract::{Download, Event, ExportFile, Failure, FailureKind, RequestId};
use export::{Block, ExportError};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use std::path::{Path, PathBuf};

pub(super) async fn write_file(
    request: RequestId,
    file: ExportFile,
    home: &Path,
    content: &Path,
    reply: &Reply,
) {
    let format = file.format;
    let home = home.to_owned();
    let content = content.to_owned();
    let written = tokio::task::spawn_blocking(move || write_download(file, &home, &content)).await;
    let result = match written {
        Ok(result) => result,
        Err(error) => Err(Failure::internal(format!("the export stopped: {error}"))),
    };
    reply.send(Event::Exported {
        request,
        format,
        result,
    });
}

fn write_download(mut file: ExportFile, home: &Path, content: &Path) -> Result<Download, Failure> {
    if file.file_name.is_empty()
        || file.file_name.contains(['/', '\\'])
        || file.file_name == "."
        || file.file_name == ".."
    {
        return Err(Failure::internal("the export file name is not a file name"));
    }
    for block in &mut file.document.blocks {
        if let Block::Figure {
            picture: Some(picture),
            ..
        } = block
        {
            *picture = resolve_picture(picture, content)?;
        }
    }
    let id = uuid::Uuid::new_v4();
    let folder = home.join("data/exports").join(id.to_string());
    std::fs::create_dir_all(&folder).map_err(|error| {
        Failure::new(
            FailureKind::WriteFailed,
            format!("could not make {}: {error}", folder.display()),
        )
        .with_hint(format!(
            "The export could not be written: {error}. Try exporting again."
        ))
    })?;
    let path = folder.join(&file.file_name);
    export::write(&file.document, file.format, &path).map_err(|error| failure(&error))?;
    Ok(Download {
        url: format!(
            "/api/download/{id}/{}",
            utf8_percent_encode(&file.file_name, NON_ALPHANUMERIC)
        ),
        file_name: file.file_name,
    })
}

fn resolve_picture(picture: &Path, content: &Path) -> Result<PathBuf, Failure> {
    let url = picture.to_string_lossy();
    let relative = url
        .strip_prefix("/content/")
        .ok_or_else(|| Failure::internal("the export picture is not a content URL"))?;
    let relative = percent_decode_str(relative)
        .decode_utf8()
        .map_err(|error| Failure::internal(format!("the picture URL is not UTF-8: {error}")))?;
    if relative.contains('\\')
        || Path::new(relative.as_ref())
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(Failure::internal(
            "the export picture leaves the content folder",
        ));
    }
    let root = content.canonicalize().map_err(|error| {
        Failure::internal(format!("could not read {}: {error}", content.display()))
    })?;
    let joined = root.join(relative.as_ref());
    let mut existing = joined.as_path();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing
                    .parent()
                    .ok_or_else(|| Failure::internal("the export picture has no content parent"))?;
            }
            Err(error) => {
                return Err(Failure::internal(format!(
                    "could not read {}: {error}",
                    existing.display()
                )));
            }
        }
    }
    let path = existing.canonicalize().map_err(|error| {
        Failure::internal(format!("could not resolve {}: {error}", existing.display()))
    })?;
    if !path.starts_with(&root) {
        return Err(Failure::internal(
            "the export picture leaves the content folder",
        ));
    }
    Ok(if existing == joined { path } else { joined })
}

fn failure(error: &ExportError) -> Failure {
    match error {
        ExportError::Write { path, source } => {
            Failure::new(FailureKind::WriteFailed, format!("{error}: {source}")).with_hint(format!(
                "{} could not be written: {source}. Try exporting again.",
                path.display()
            ))
        }
        ExportError::Pdf { reason } => Failure::internal(error.to_string()).with_hint(format!(
            "The PDF could not be made: {reason}. Try again, or export as HTML."
        )),
    }
}
