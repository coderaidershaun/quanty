//! The documents saved under one content root: `<root>/<media folder>/<document folder>`.

use std::path::{Path, PathBuf};

use super::{CHAPTER_INDEX_FILE, ChapterIndex, ContentError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterEntry {
    /// `<root>/<media folder>/<document folder>`. Absolute when the root was.
    pub folder: PathBuf,
    pub index: ChapterIndex,
}

#[derive(Debug, Default)]
pub struct Catalogue {
    /// By media folder, then chapters by number, then titled documents by title.
    pub chapters: Vec<ChapterEntry>,
    /// One error for each `chapter.json` that is there and cannot be read.
    pub unreadable: Vec<ContentError>,
}

impl Catalogue {
    /// A root that does not exist holds no chapter, and a chapter that cannot be read does not
    /// hide the others.
    ///
    /// # Errors
    /// [`ContentError::Read`] when the root or a media folder is there and cannot be listed.
    pub fn read(content_root: &Path) -> Result<Catalogue, ContentError> {
        let mut catalogue = Catalogue::default();
        for media_folder in folders_in(content_root)? {
            for chapter_folder in folders_in(&media_folder)? {
                // A folder of pictures that stand alone holds no `chapter.json`.
                if !chapter_folder.join(CHAPTER_INDEX_FILE).is_file() {
                    continue;
                }
                match ChapterIndex::read(&chapter_folder) {
                    Ok(index) => catalogue.chapters.push(ChapterEntry {
                        folder: chapter_folder,
                        index,
                    }),
                    Err(error) => catalogue.unreadable.push(error),
                }
            }
        }
        catalogue.chapters.sort_by(|a, b| {
            (a.folder.parent(), &a.index.name).cmp(&(b.folder.parent(), &b.index.name))
        });
        Ok(catalogue)
    }
}

/// The folders directly in `folder` by name, or none when `folder` is not there.
fn folders_in(folder: &Path) -> Result<Vec<PathBuf>, ContentError> {
    let read_error = |source| ContentError::Read {
        path: folder.to_path_buf(),
        source,
    };
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(read_error(source)),
    };
    let mut folders = Vec::new();
    for entry in entries {
        let path = entry.map_err(read_error)?.path();
        if path.is_dir() {
            folders.push(path);
        }
    }
    folders.sort();
    Ok(folders)
}
