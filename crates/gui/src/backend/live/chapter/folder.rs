//! Finds the folder of a stored document's converted chapter on disk, and says why when it cannot
//! be used, for every part of the live backend that opens a chapter.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ocr::content::CHAPTER_INDEX_FILE;
use ocr::{Catalogue, ChapterEntry, ChapterIndex, ContentError};
use rag_core::DocId;

use crate::backend::live::{LiveContext, Services};
use crate::contract::Failure;

/// Why the folder of a document's chapter could not be used.
#[derive(Debug)]
pub(super) enum FolderFault {
    /// Neither the folder that was given nor a scan of the content folder holds the chapter.
    Unknown,
    /// The folder was found and could not be made canonical.
    Unreadable(ContentError),
}

impl FolderFault {
    pub(super) fn failure<S: Services>(self, cx: &LiveContext<S>, doc: DocId) -> Failure {
        match self {
            FolderFault::Unknown => {
                let config = cx.config();
                let content_folder = &config.content_folder;
                let searched = content_folder.display();
                Failure {
                    detail: format!(
                        "no folder was found for document {}: none was given that holds a {CHAPTER_INDEX_FILE}, and no converted folder that could be read under {searched} is its own",
                        doc.0
                    ),
                    ..Failure::pages_not_found(content_folder)
                }
            }
            FolderFault::Unreadable(error) => cx.failure(error),
        }
    }
}

/// The folder of a stored document's chapter: the one given when it holds a `chapter.json`, else
/// the one a scan of `content_folder` finds. It is made canonical, as ingestion does, so a picture
/// has one path whichever way the folder was found, and one place in the cache of decoded
/// pictures.
pub(super) fn chapter_folder(
    doc: DocId,
    given: Option<&Path>,
    content_folder: &Path,
) -> Result<PathBuf, FolderFault> {
    let stored = given.filter(|folder| folder.join(CHAPTER_INDEX_FILE).is_file());
    let folder = match stored {
        Some(folder) => folder.to_path_buf(),
        None => {
            let mut found = chapters_on_disk([(doc, None)], content_folder);
            found
                .remove(&doc)
                .map(|entry| entry.folder)
                .ok_or(FolderFault::Unknown)?
        }
    };
    std::fs::canonicalize(&folder).map_err(|source| {
        FolderFault::Unreadable(ContentError::Read {
            path: folder,
            source,
        })
    })
}

/// Only a finished chapter is found, so a conversion that stopped half way never hides a whole
/// copy of the same PDF. A stored folder is used only when its `chapter.json` reads and names
/// this document: a folder that now holds another document is not this document's folder. The
/// scan of `content_root` finds a document that was stored before folders were kept.
pub(in crate::backend::live) fn chapters_on_disk<'a>(
    documents: impl IntoIterator<Item = (DocId, Option<&'a Path>)>,
    content_root: &Path,
) -> HashMap<DocId, ChapterEntry> {
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
    let is_this_document = DocId::from_source_sha256(&index.source_sha256) == id;
    (index.finished && is_this_document).then(|| ChapterEntry {
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
    catalogue
        .chapters
        .into_iter()
        .filter(|entry| entry.index.finished)
        .collect()
}
