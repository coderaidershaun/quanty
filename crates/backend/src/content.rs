//! Stored picture paths become URLs relative to the one content folder the server serves.

use std::path::Path;

pub(crate) fn image_url(path: &Path, root: &Path) -> Option<String> {
    rag_core::content_url(path, root).map(|url| format!("/{url}"))
}
