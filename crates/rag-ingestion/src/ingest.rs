//! Takes one converted chapter folder from disk to stored points and graph nodes: read, map to
//! items, write the graph, embed, store. Running it again on the same chapter writes the same
//! points and the same nodes over themselves.

use std::fmt;
use std::path::{Path, PathBuf};

use graph::{DocumentNode, GraphError, GraphStore, ItemNode};
use ocr::ReadChapterError;
use rag_core::{DocId, DocumentInput, EmbedError, Embedder, ItemKind, ItemPoint, StoreError};

use crate::items::{Item, chapter_items, document_id, document_title};
use crate::stores::Stores;

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

    #[error("could not write the chapter to the graph")]
    Graph(#[from] GraphError),

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

/// Reads the chapter in `chapter_folder`, writes its document and items to the graph, embeds the
/// items and stores them as points.
///
/// The collection is prepared and the graph is written before anything is embedded, so a store
/// that is down fails the run before an embedding call is paid for. A run that stops after that
/// leaves the chapter in the graph with no points yet, and running it again stores them. A point
/// is never stored without its node. Every stored picture path is absolute.
///
/// # Errors
/// - [`IngestError::ChapterFolder`] when the folder does not exist
/// - [`IngestError::Read`] when it is not a finished converted chapter
/// - [`IngestError::Embed`], [`IngestError::Store`] and [`IngestError::Graph`] when the outside
///   calls fail
/// - [`IngestError::VectorCount`] when the embedder returns another number of vectors than
///   there were items
pub async fn ingest_chapter<G: GraphStore>(
    chapter_folder: &Path,
    embedder: &impl Embedder,
    stores: &Stores<G>,
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
    let document = DocumentNode {
        id: document_id(&chapter.index),
        title: document_title(&chapter.index),
    };
    let nodes: Vec<ItemNode> = items.iter().map(item_node).collect();

    stores.items.ensure_collection().await?;
    // SMELL: nothing removes the item nodes of an earlier run either, with their `NEXT` edges. A
    // chapter that is cut into items differently ends up with two chains of items in the graph,
    // until its document is deleted and ingested again.
    stores.graph.upsert_document(&document).await?;
    stores.graph.upsert_items(document.id, &nodes).await?;

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
    stores.items.upsert(&points).await?;
    let points_in_collection = stores.items.count().await?;

    Ok(IngestSummary {
        doc_id: document.id,
        doc_title: document.title,
        collection: stores.items.collection().to_owned(),
        items_by_kind,
        points_in_collection,
    })
}

fn item_node(item: &Item) -> ItemNode {
    ItemNode {
        id: item.id,
        kind: item.payload.kind,
        page: item.payload.page,
        printed_page: item.payload.printed_page.clone(),
    }
}
