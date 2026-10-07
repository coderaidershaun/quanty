//! Finds the folder of a stored document's converted chapter on disk, for every part of the live
//! backend that opens a chapter.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use ocr::{Catalogue, ChapterEntry, ChapterIndex};
use rag_core::DocId;

/// The converted chapter of each document, read from disk only. A document is given with the
/// folder the graph stored for it, if any. A stored folder is used when its `chapter.json` reads
/// and names this document: a folder that now holds another document is not this document's
/// folder. Each document that is left is looked for by its id among the chapters under
/// `content_root`, which is how a document stored before folders were kept is found.
///
/// A document that is found by neither way has no entry. A content folder that cannot be listed,
/// or a chapter under it that cannot be read, is logged and never fails the lookup.
pub(super) fn chapters_on_disk<'a>(
    documents: impl IntoIterator<Item = (DocId, Option<&'a Path>)>,
    content_root: &Path,
) -> HashMap<DocId, ChapterEntry> {
    // SMELL: a chapter that is not finished is found like a finished one, so an entry does not
    // promise that the chapter can be read. A caller learns that only when it reads the chapter.
    let mut found = HashMap::new();
    let mut missing = HashSet::new();
    for (id, stored_folder) in documents {
        match stored_folder.and_then(|folder| chapter_of(id, folder)) {
            Some(entry) => {
                found.insert(id, entry);
            }
            None => {
                missing.insert(id);
            }
        }
    }
    if !missing.is_empty() {
        for entry in chapters_under(content_root) {
            let id = DocId::from_source_sha256(&entry.index.source_sha256);
            if missing.contains(&id) {
                found.entry(id).or_insert(entry);
            }
        }
    }
    found
}

fn chapter_of(id: DocId, folder: &Path) -> Option<ChapterEntry> {
    let index = ChapterIndex::read(folder).ok()?;
    (DocId::from_source_sha256(&index.source_sha256) == id).then(|| ChapterEntry {
        folder: folder.to_path_buf(),
        index,
    })
}

fn chapters_under(content_root: &Path) -> Vec<ChapterEntry> {
    let catalogue = match Catalogue::read(content_root) {
        Ok(catalogue) => catalogue,
        Err(error) => {
            tracing::warn!(?error, "could not list the converted chapters on disk");
            return Vec::new();
        }
    };
    if !catalogue.unreadable.is_empty() {
        tracing::warn!(
            count = catalogue.unreadable.len(),
            "some converted chapters on disk could not be read"
        );
    }
    catalogue.chapters
}
