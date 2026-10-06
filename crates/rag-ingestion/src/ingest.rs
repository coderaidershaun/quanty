//! Takes one converted chapter folder from disk to stored points, graph nodes and concepts: read,
//! map to items, write the graph, embed, store, extract the concepts. Running it again on the
//! same chapter writes the same points and the same nodes over themselves, and asks the language
//! model nothing.

use std::fmt;
use std::path::{Path, PathBuf};

use graph::{DocumentNode, GraphError, GraphStore, ItemNode};
use ocr::ReadChapterError;
use rag_core::{DocId, DocumentInput, EmbedError, Embedder, ItemKind, ItemPoint, Llm, StoreError};

use crate::concepts::{ConceptError, ConceptExtractor, ConceptSummary};
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

    #[error("could not extract the concepts of the chapter")]
    Concepts(#[from] ConceptError),
}

/// The two outside models an ingest pays for.
pub struct Models<E, L> {
    pub embedder: E,
    pub concepts: ConceptExtractor<L>,
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
    pub concepts: ConceptSummary,
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
        writeln!(
            formatter,
            "points now in collection {}: {}",
            self.collection, self.points_in_collection
        )?;
        let concepts = &self.concepts;
        writeln!(
            formatter,
            "concepts: {} created, {} linked to an existing concept",
            concepts.concepts_created, concepts.concepts_linked
        )?;
        writeln!(formatter, "mentions written: {}", concepts.mentions_written)?;
        writeln!(
            formatter,
            "relations written: {}, dropped: {}",
            concepts.relations_written, concepts.relations_dropped
        )?;
        writeln!(
            formatter,
            "claude calls made: {}, cache hits: {}",
            concepts.llm_calls, concepts.cache_hits
        )?;
        write!(formatter, "items skipped: {}", concepts.skipped_items.len())?;
        for skipped in &concepts.skipped_items {
            write!(
                formatter,
                "\n  {} on page {} ({}): {}",
                skipped.kind.as_str(),
                skipped.page,
                skipped.id,
                skipped.reason
            )?;
        }
        Ok(())
    }
}

/// Reads the chapter in `chapter_folder`, writes its document and items to the graph, embeds the
/// items, stores them as points, and then writes the concepts that the items discuss to the
/// graph.
///
/// The collection is prepared and the graph is written before anything is embedded, so a store
/// that is down fails the run before an embedding call is paid for. A run that stops after that
/// leaves the chapter in the graph with no points yet, and running it again stores them. A point
/// is never stored without its node. Every stored picture path is absolute.
///
/// The concepts come last, after the points are stored, so a chapter whose extraction failed can
/// still be searched. Extraction that stops, for example because `claude` has reached its usage
/// limit or is not signed in, keeps the answers it has so far, and running the same command again
/// goes on from them.
///
/// # Errors
/// - [`IngestError::ChapterFolder`] when the folder does not exist
/// - [`IngestError::Read`] when it is not a finished converted chapter
/// - [`IngestError::Embed`], [`IngestError::Store`] and [`IngestError::Graph`] when the outside
///   calls fail
/// - [`IngestError::VectorCount`] when the embedder returns another number of vectors than
///   there were items
/// - [`IngestError::Concepts`] when extraction stops or cannot use its cache folder or the graph.
///   An item whose questions fail is not an error: the summary names it.
pub async fn ingest_chapter<E: Embedder, L: Llm, G: GraphStore>(
    chapter_folder: &Path,
    models: &Models<E, L>,
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

    let inputs: Vec<DocumentInput> = items.iter().map(|item| item.input.clone()).collect();
    let vectors = models.embedder.embed_document(&inputs).await?;
    if vectors.len() != items.len() {
        return Err(IngestError::VectorCount {
            items: items.len(),
            vectors: vectors.len(),
        });
    }

    let points: Vec<ItemPoint> = items
        .iter()
        .zip(vectors)
        .map(|(item, vector)| ItemPoint {
            id: item.id,
            vector,
            payload: item.payload.clone(),
        })
        .collect();
    // SMELL: nothing removes the points of an earlier run. When the way a chapter is cut into
    // items changes, its items get other positions and so other identifiers, and the old points
    // of the chapter stay in the collection beside the new ones.
    stores.items.upsert(&points).await?;
    let points_in_collection = stores.items.count().await?;

    // SMELL: a `claude` that cannot answer at all, because a key is set, it is not signed in or
    // it is not installed, is found only here, after the embedding of the whole chapter was paid
    // for. Running the command again pays for the embedding again.
    let concepts = models.concepts.extract(&items, &stores.graph).await?;

    Ok(IngestSummary {
        doc_id: document.id,
        doc_title: document.title,
        collection: stores.items.collection().to_owned(),
        items_by_kind,
        points_in_collection,
        concepts,
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
