//! What the content folder holds of one document, and the removal of it. A file is found only by
//! listing the content folder it is given, never by a path that a store names.

use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use ocr::content::media_folder_name;
use ocr::{
    Catalogue, ChapterEntry, ChapterLock, ContentError, ImageEntry, saved_images, sha256_hex,
    try_lock_chapter,
};
use rag_core::DocId;

use super::DeleteError;

// SMELL: the MCP server keeps its own copy of this name, and the two must be the same.
const UPLOADS_FOLDER: &str = "_uploads";

/// The converted folders of one document that the content folder holds, each held so that no
/// conversion starts in it until the folders are removed. A picture that stands alone has no
/// conversion to hold.
pub(super) struct DocumentFiles {
    chapters: Vec<ChapterEntry>,
    pictures: Vec<ImageEntry>,
    _locks: Vec<ChapterLock>,
}

/// What a removal took out of the content folder.
pub(super) struct RemovedFiles {
    pub(super) folders: Vec<PathBuf>,
    pub(super) uploads: Vec<PathBuf>,
}

impl DocumentFiles {
    /// Lists the content folder and keeps every converted folder whose index gives this document
    /// id, finished or not. A folder whose `chapter.json` cannot be read is not provably this
    /// document's, so it is left out. It removes nothing.
    ///
    /// A folder that a conversion holds at this moment is refused, so that the pages of a run
    /// that was paid for are not removed.
    pub(super) fn find(id: DocId, content_folder: &Path) -> Result<DocumentFiles, DeleteError> {
        // HAZARD: the listing takes a symbolic link to a folder as a folder. When a media folder
        // is a link, the document folders in the place it points to are removed, and that place
        // can be outside the content folder. quanty makes no such link.
        let chapters: Vec<ChapterEntry> = Catalogue::read(content_folder)?
            .chapters
            .into_iter()
            .filter(|entry| DocId::from_source_sha256(&entry.index.source_sha256) == id)
            .collect();
        let pictures = saved_images(content_folder)?
            .into_iter()
            .filter(|entry| DocId::from_source_sha256(&entry.index.source_sha256) == id)
            .collect();
        // SMELL: only a conversion holds a folder. An ingest that goes on after its conversion
        // holds nothing, so a delete during that ingest is not refused, and the ingest can then
        // store points of a document whose nodes and folder are gone.
        let mut locks = Vec::new();
        for entry in &chapters {
            let lock = try_lock_chapter(&entry.folder)?.ok_or_else(|| DeleteError::Converting {
                folder: entry.folder.clone(),
            })?;
            locks.push(lock);
        }
        Ok(DocumentFiles {
            chapters,
            pictures,
            _locks: locks,
        })
    }

    /// Removes the upload copy of each chapter and then its folder, and then the folder of each
    /// picture. The copy goes first because `chapter.json` in the folder is the only thing that
    /// names it: a run that stops between the two leaves the folder, and the next run finds it.
    /// Every chapter folder stays held until this returns.
    pub(super) fn remove(self, content_folder: &Path) -> Result<RemovedFiles, DeleteError> {
        let mut removed = RemovedFiles {
            folders: Vec::new(),
            uploads: Vec::new(),
        };
        for entry in self.chapters {
            if let Some(upload) = remove_upload(&entry, content_folder)? {
                removed.uploads.push(upload);
            }
            remove_folder(&entry.folder)?;
            removed.folders.push(entry.folder);
        }
        for picture in self.pictures {
            remove_folder(&picture.folder)?;
            removed.folders.push(picture.folder);
        }
        Ok(removed)
    }
}

/// Removes the folder of the media and its folder of uploads, each only when nothing is left in
/// it, and gives the ones it removed. Two titles that differ only in punctuation name one folder,
/// so a folder that still holds a document of the other title stays. A title that names no
/// folder removes nothing.
pub(super) fn remove_empty_media_folders(
    title: &str,
    content_folder: &Path,
) -> Result<Vec<PathBuf>, DeleteError> {
    let Ok(media_folder) = media_folder_name(title) else {
        return Ok(Vec::new());
    };
    let folders = [
        content_folder.join(&media_folder),
        content_folder.join(UPLOADS_FOLDER).join(&media_folder),
    ];
    let mut removed = Vec::new();
    for folder in folders {
        // Never use `remove_dir_all` here. A media folder can still hold the documents of a
        // title that differs only in punctuation, and `remove_dir` leaves any folder that has
        // something in it.
        match fs::remove_dir(&folder) {
            Ok(()) => removed.push(folder),
            Err(source)
                if matches!(
                    source.kind(),
                    ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(source) => {
                return Err(DeleteError::Remove {
                    path: folder,
                    source,
                });
            }
        }
    }
    Ok(removed)
}

fn remove_folder(folder: &Path) -> Result<(), DeleteError> {
    // SMELL: a removal that is cut short can leave a part of the folder without its index file,
    // and no delete finds that part again. A page that kept its `page.json` and lost a piece file
    // can then stop a later ingest of the same PDF until a person removes the folder.
    fs::remove_dir_all(folder).map_err(|source| DeleteError::Remove {
        path: folder.to_path_buf(),
        source,
    })
}

/// Removes the copy of the PDF of this chapter, and says which file it removed. The copy is kept
/// as `<content folder>/_uploads/<media folder>/<source file>`. It goes only when it is a file
/// with the same bytes as the PDF that the chapter was made from: the name alone does not prove
/// that.
fn remove_upload(
    entry: &ChapterEntry,
    content_folder: &Path,
) -> Result<Option<PathBuf>, DeleteError> {
    let source_file = entry.index.source_file.as_str();
    let is_plain_file_name = Path::new(source_file).file_name() == Some(OsStr::new(source_file));
    if !is_plain_file_name {
        return Ok(None);
    }
    let Some(media_folder) = entry.folder.parent().and_then(Path::file_name) else {
        return Ok(None);
    };
    let upload = content_folder
        .join(UPLOADS_FOLDER)
        .join(media_folder)
        .join(source_file);
    if !upload.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&upload).map_err(|source| ContentError::Read {
        path: upload.clone(),
        source,
    })?;
    if sha256_hex(&bytes) != entry.index.source_sha256 {
        return Ok(None);
    }
    fs::remove_file(&upload).map_err(|source| DeleteError::Remove {
        path: upload.clone(),
        source,
    })?;
    Ok(Some(upload))
}
