//! Stored picture paths become URLs relative to the one content folder the server serves.

use std::path::{Component, Path};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub(crate) fn image_url(path: &Path, root: &Path) -> Option<String> {
    let root = std::path::absolute(root).ok()?;
    let path = std::path::absolute(path).ok()?;
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
    Some(format!("/content/{}", parts.join("/")))
}
