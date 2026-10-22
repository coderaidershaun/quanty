//! Uploaded source PDFs stay beside converted content so a document can be read and removed later.

use std::io;
use std::path::{Component, Path, PathBuf};

pub(crate) const UPLOADS_FOLDER: &str = "_uploads";

#[derive(Debug, thiserror::Error)]
#[error("{name:?} must be a plain file name without a folder")]
pub struct UploadNameError {
    pub name: String,
}

/// Rejects separators from both server and browser platforms.
///
/// # Errors
/// Returns [`UploadNameError`] when a name is empty, a folder or a path.
pub fn check_upload_file_name(name: &str) -> Result<(), UploadNameError> {
    let mut parts = Path::new(name).components();
    let plain = matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none();
    if !plain || name.contains(['/', '\\', '\0']) {
        return Err(UploadNameError { name: name.into() });
    }
    Ok(())
}

/// The media folder is the name produced by `ocr::content::media_folder_name`.
///
/// # Errors
/// Returns [`UploadNameError`] if either folder or file name holds a path.
pub fn uploaded_pdf_path(
    content_folder: &Path,
    media_folder: &str,
    file_name: &str,
) -> Result<PathBuf, UploadNameError> {
    check_upload_file_name(media_folder)?;
    check_upload_file_name(file_name)?;
    Ok(content_folder
        .join(UPLOADS_FOLDER)
        .join(media_folder)
        .join(file_name))
}

/// Saves the uploaded source beside its converted content.
///
/// # Errors
/// Returns the file error when its folder or bytes cannot be written.
pub fn save_uploaded_pdf(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, bytes)
}
