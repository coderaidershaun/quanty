//! Stored picture paths become URLs relative to the one content folder the server serves.

use std::path::{Component, Path, PathBuf};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Resolve a picture under the content folder to a URL relative to the app base.
/// Paths outside that folder cannot become public references.
pub fn content_url(path: &Path, root: &Path) -> Option<String> {
    let root = normalized(root)?;
    let path = normalized(path)?;
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return None;
        };
        parts.push(utf8_percent_encode(part.to_str()?, PATH_SEGMENT).to_string());
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("content/{}", parts.join("/")))
}

fn normalized(path: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(path).ok()?;
    let mut normalized = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            part => normalized.push(part.as_os_str()),
        }
    }
    Some(normalized)
}
