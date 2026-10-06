//! Takes one converted chapter folder from disk to stored points: read, map to items, embed,
//! store. Running it again on the same chapter writes the same points over themselves.

use std::fmt;
use std::path::{Path, PathBuf};

use ocr::ReadChapterError;
use rag_core::{
    DocId, DocumentInput, EmbedError, Embedder, ItemKind, ItemPoint, ItemStore, StoreError,
};

use crate::items::{Item, chapter_items, document_id, document_title};

#[derive(thiserror::Error, Debug)]
pub enum IngestError {
    #[error("the chapter folder {} cannot be found", path.display())]
    ChapterFolder {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read the converted chapter")]
    Read(#[from] ReadChapterError),

    #[error("could not embed the items of the chapter")]
    Embed(#[from] EmbedError),

    #[error("could not store the items of the chapter")]
    Store(#[from] StoreError),

    #[error("the chapter made {items} items but the embedder returned {vectors} vectors")]
    VectorCount { items: usize, vectors: usize },
}

/// How many items of each kind a chapter made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemCounts {
    pub chunks: usize,
    pub formulas: usize,
    pub figures: usize,
    pub tables: usize,
}

impl ItemCounts {
    pub fn total(&self) -> usize {
        self.chunks + self.formulas + self.figures + self.tables
    }

    fn of(items: &[Item]) -> Self {
        let mut counts = Self::default();
        for item in items {
            match item.payload.kind {
                ItemKind::Chunk => counts.chunks += 1,
                ItemKind::Formula => counts.formulas += 1,
                ItemKind::Figure => counts.figures += 1,
                ItemKind::Table => counts.tables += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestSummary {
    pub doc_id: DocId,
    pub doc_title: String,
    pub collection: String,
    pub items_by_kind: ItemCounts,
    /// The count of the whole collection after the run, not only of this chapter.
    pub points_in_collection: u64,
}

impl fmt::Display for IngestSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = &self.items_by_kind;
        writeln!(formatter, "document: {}", self.doc_title)?;
        writeln!(formatter, "document id: {}", self.doc_id)?;
        writeln!(
            formatter,
            "items: {} chunks, {} formulas, {} figures, {} tables ({} in all)",
            counts.chunks,
            counts.formulas,
            counts.figures,
            counts.tables,
            counts.total()
        )?;
        write!(
            formatter,
            "points now in collection {}: {}",
            self.collection, self.points_in_collection
        )
    }
}

/// Reads the chapter in `chapter_folder`, embeds its items and stores them as points.
///
/// The store is prepared before anything is embedded, so a store that is down fails the run
/// before an embedding call is paid for. Every stored picture path is absolute.
///
/// # Errors
/// - [`IngestError::ChapterFolder`] when the folder does not exist
/// - [`IngestError::Read`] when it is not a finished converted chapter
/// - [`IngestError::Embed`] and [`IngestError::Store`] when the outside calls fail
/// - [`IngestError::VectorCount`] when the embedder returns another number of vectors than
///   there were items
pub async fn ingest_chapter(
    chapter_folder: &Path,
    embedder: &impl Embedder,
    store: &ItemStore,
) -> Result<IngestSummary, IngestError> {
    // SMELL: the stored picture paths are absolute, so they stop working when the chapter
    // folder is moved, until the chapter is ingested again.
    let folder =
        std::fs::canonicalize(chapter_folder).map_err(|source| IngestError::ChapterFolder {
            path: chapter_folder.to_path_buf(),
            source,
        })?;
    let chapter = ocr::read_chapter(&folder)?;
    let items = chapter_items(&chapter);
    let items_by_kind = ItemCounts::of(&items);

    store.ensure_collection().await?;

    let (inputs, stored): (Vec<DocumentInput>, Vec<_>) = items
        .into_iter()
        .map(|item| (item.input, (item.id, item.payload)))
        .unzip();
    let vectors = embedder.embed_document(&inputs).await?;
    if vectors.len() != stored.len() {
        return Err(IngestError::VectorCount {
            items: stored.len(),
            vectors: vectors.len(),
        });
    }

    let points: Vec<ItemPoint> = stored
        .into_iter()
        .zip(vectors)
        .map(|((id, payload), vector)| ItemPoint {
            id,
            vector,
            payload,
        })
        .collect();
    // SMELL: nothing removes the points of an earlier run. When the way a chapter is cut into
    // items changes, its items get other positions and so other identifiers, and the old points
    // of the chapter stay in the collection beside the new ones.
    store.upsert(&points).await?;
    let points_in_collection = store.count().await?;

    Ok(IngestSummary {
        doc_id: document_id(&chapter.index),
        doc_title: document_title(&chapter.index),
        collection: store.collection().to_owned(),
        items_by_kind,
        points_in_collection,
    })
}
