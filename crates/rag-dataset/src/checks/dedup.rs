//! Remembers the sections that were kept, by the hash of their text, so that a later copy of one
//! is found.

use std::collections::HashMap;

/// What the checks have seen of the sections so far, so that a copy of a section is found.
#[derive(Debug, Default)]
pub struct Seen {
    /// The id of the first kept section, by the hash of its text.
    first_id_by_sha256: HashMap<String, String>,
}

impl Seen {
    /// The id of the kept section whose text has this hash.
    pub(super) fn first_with(&self, sha256: &str) -> Option<&str> {
        self.first_id_by_sha256.get(sha256).map(String::as_str)
    }

    pub(super) fn remember(&mut self, sha256: String, section_id: &str) {
        self.first_id_by_sha256
            .entry(sha256)
            .or_insert_with(|| section_id.to_owned());
    }
}
