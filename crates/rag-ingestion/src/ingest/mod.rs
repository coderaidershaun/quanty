//! Takes one converted chapter folder, or one picture that stands alone, from disk to stored
//! points, graph nodes and concepts, and a chapter to its saved insights. Running it again on the
//! same chapter or picture replaces what the earlier run stored, and asks the language model only
//! for what is not kept or saved yet.

mod artifacts;
mod concepts;
mod insights;
mod items;
mod summary;

use std::path::{Path, PathBuf};

use graph::{DocumentNode, GraphError, ItemNode, MediaNode};
use ocr::{DocumentName, ReadChapterError, gets_insights};
use rag_core::{
    DocumentInput, DocumentLabels, EmbedError, Embedder, Embedding, ItemPoint, ItemStore, Llm,
    LlmError, MediaLabels, StoreError, UsageTally, embedding_usage,
};

pub use artifacts::{
    ArtifactAssessment, ArtifactError, ArtifactState, ArtifactSummary, ArtifactWriter,
    assess_artifacts,
};
pub(crate) use concepts::TRIES_FOR_ONE_QUESTION;
pub use concepts::{
    ASK_SCORE, CALLS_AT_A_TIME, Cache, ConceptError, ConceptExtractor, ConceptSummary, LINK_SCORE,
    Outcome, Reading, SkippedItem,
};
#[cfg(feature = "testing")]
pub(crate) use insights::INSIGHTS_PROMPT;
pub use insights::{
    ChapterChoice, InsightsNeed, InsightsOutcome, InsightsSummary, InsightsWriter, StoredChapter,
    StoredChaptersError, stored_chapters,
};
pub use items::{
    Item, LoneImage, chapter_items, context_line, document_title, document_title_of, image_items,
};
pub use summary::{IngestStep, IngestSummary, ItemCounts, usage_of};
pub(crate) use summary::{Meter, OnStep, write_usage};

use concepts::EmbeddedItems;
use items::document_id;

use crate::media::stored_or_added;
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

    #[error(
        "claude cannot be asked for the concepts of the document, so nothing was embedded or stored"
    )]
    ModelNotReady(#[source] LlmError),

    #[error("could not embed the items of the document")]
    Embed(#[from] EmbedError),

    #[error("could not store the items of the document")]
    Store(#[from] StoreError),

    #[error("could not read or write the document in the graph")]
    Graph(#[from] GraphError),

    #[error("the document made {items} items but the embedder returned {vectors} vectors")]
    VectorCount { items: usize, vectors: usize },

    #[error("could not extract the concepts of the document")]
    Concepts(#[from] ConceptError),

    #[error("required artifact metadata could not be completed")]
    Artifacts(#[from] ArtifactError),
}

/// The outside models an ingest pays for.
pub struct Models<E, L> {
    pub embedder: E,
    /// Required metadata for every chapter-backed artifact.
    pub artifacts: ArtifactWriter<L>,
    pub concepts: ConceptExtractor<L>,
    /// Optional insights for eligible chapters; required artifact descriptions are independent.
    pub insights: Option<InsightsWriter<L>>,
}

/// A converted chapter folder, and the labels its media is made with when the graph does not
/// have that media yet.
#[derive(Debug, Clone, Copy)]
pub struct ChapterFolder<'a> {
    pub folder: &'a Path,
    pub new_media: &'a MediaLabels,
}

struct Document {
    node: DocumentNode,
    items: Vec<Item>,
    /// The media of a chapter, with the labels a new media is made with. A picture that stands
    /// alone has none.
    media: Option<MediaNode>,
    /// What making the document used before the ingest: the conversion of a picture. A chapter's
    /// conversion is counted by its caller.
    spent: UsageTally,
    /// The converted folder of a chapter. A picture that stands alone has none, and gets no
    /// insights.
    chapter: Option<ConvertedChapter>,
}

/// Where a chapter was converted to, and how it is named.
struct ConvertedChapter {
    folder: PathBuf,
    name: DocumentName,
}

/// The language model is asked first whether it can answer, which costs nothing, so a `claude`
/// that is not signed in fails the run before anything is written or paid for.
///
/// The collections are prepared and the graph is written before anything is embedded, so a store
/// that is down fails the run before an embedding call is paid for. What an earlier run stored of
/// the document is removed first, so no item, node or mention that this run does not make is left
/// behind. A run that stops after that leaves the chapter in the graph with no points yet, and
/// running it again stores them. A point is never stored without its node. Every stored picture
/// path is absolute.
///
/// Required artifact titles and descriptions are completed before any earlier points or nodes
/// are replaced. A failed metadata request retains accepted checkpoints and prior search data.
/// They are Library display fields and never change embedding or concept inputs.
///
/// The insights, the takeaways and the cheat sheet of a chapter are written after the points are
/// stored, and only when the models hold an insights writer and the chapter's folder holds no
/// saved file. A part of a book that is not a chapter, such as its index, gets none: the summary
/// says so and the model is not asked. A failed answer is not an error: the summary says why
/// nothing was written, and the items stay stored.
///
/// The concepts come last, after the points are stored, so a chapter whose extraction failed can
/// still be searched. Extraction that stops, for example because `claude` has reached its usage
/// limit or is not signed in, keeps the answers it has so far, and running the same command again
/// goes on from them.
///
/// The document node and every point carry the labels of the chapter's media: the stored media
/// of that title, whatever its capitals, or else a new one that is made from `new_media` and
/// added to the graph. An ingest never changes a stored media. The own tags of the document are
/// the ones the stored node already has, so an ingest never removes one. The labels are not
/// embedded.
///
/// The document node carries a mark that it is ingested whole, with the number of its items. It
/// is taken away when prior store data is replaced and set as the last step, only when no item was
/// skipped, so the mark is never there for a run that stopped or skipped an item.
///
/// # Errors
/// - [`IngestError::ChapterFolder`] when the folder does not exist
/// - [`IngestError::Read`] when it is not a finished converted chapter
/// - [`IngestError::ModelNotReady`] when the language model cannot be asked anything until the
///   person acts
/// - [`IngestError::Embed`], [`IngestError::Store`] and [`IngestError::Graph`] when the outside
///   calls fail
/// - [`IngestError::VectorCount`] when the embedder returns another number of vectors than
///   there were items
/// - [`IngestError::Artifacts`] when required metadata cannot be completed or saved
/// - [`IngestError::Concepts`] when extraction stops, cannot compare two concepts, or cannot use
///   its cache folder, its decision log or a store. An item whose questions fail is not an error:
///   the summary names it.
pub async fn ingest_chapter<E: Embedder, L: Llm>(
    chapter: ChapterFolder<'_>,
    models: &Models<E, L>,
    stores: &Stores,
) -> Result<IngestSummary, IngestError> {
    ingest_chapter_reporting(chapter, models, stores, &mut |_, _| {}).await
}

/// [`ingest_chapter`], telling each step to `on_step` as it happens.
pub(crate) async fn ingest_chapter_reporting<E: Embedder, L: Llm>(
    chapter: ChapterFolder<'_>,
    models: &Models<E, L>,
    stores: &Stores,
    on_step: OnStep<'_>,
) -> Result<IngestSummary, IngestError> {
    // SMELL: the stored picture paths are absolute, so they stop working when the chapter folder is
    // moved, until it is ingested again. Every program that shows a picture opens the stored path.
    let folder =
        std::fs::canonicalize(chapter.folder).map_err(|source| IngestError::ChapterFolder {
            path: chapter.folder.to_path_buf(),
            source,
        })?;
    let converted = ocr::read_chapter(&folder)?;
    let document = Document {
        node: DocumentNode {
            id: document_id(&converted.index),
            title: document_title(&converted.index),
            labels: DocumentLabels::default(),
        },
        items: chapter_items(&converted),
        media: Some(MediaNode {
            title: converted.index.media_title.trim().to_owned(),
            labels: chapter.new_media.clone(),
        }),
        spent: UsageTally::default(),
        chapter: Some(ConvertedChapter {
            folder,
            name: converted.index.name.clone(),
        }),
    };
    ingest_items(document, models, stores, on_step).await
}

/// Ingests a picture that stands alone as a document of one figure, in the same steps and with
/// the same guarantees as [`ingest_chapter`]. The document id is made from the bytes of the
/// picture, so the same picture with another note, or with none, writes over the same document.
/// The note is part of the text that is asked about, so another note asks the model again, and
/// the mentions that the earlier note led to are removed.
///
/// # Errors
/// The same as [`ingest_chapter`], except that there is no chapter to find or read.
pub async fn ingest_image<E: Embedder, L: Llm>(
    picture: &LoneImage<'_>,
    models: &Models<E, L>,
    stores: &Stores,
) -> Result<IngestSummary, IngestError> {
    // SMELL: the stored picture path is absolute, so it stops working when the converted picture is
    // moved, until it is ingested again. Every program that shows a picture opens the stored path.
    let items = image_items(picture);
    let document = Document {
        node: DocumentNode {
            id: items[0].payload.doc_id,
            title: items[0].payload.doc_title.clone(),
            labels: DocumentLabels::default(),
        },
        items,
        media: None,
        spent: usage_of(&picture.image.calls),
        chapter: None,
    };
    ingest_items(document, models, stores, &mut |_, _| {}).await
}

impl Document {
    /// Gives the node and every item the labels of the media, the stored media of that title or
    /// else a new one that is added, and the own tags that the stored node already has.
    async fn take_labels(&mut self, stores: &Stores) -> Result<(), GraphError> {
        if let Some(media) = self.media.take() {
            let media = stored_or_added(media, stores).await?;
            self.node.labels.take_media(&media.title, &media.labels);
        }
        // An ingest never removes an own tag that was set after an earlier one.
        if let Some(stored) = stores.graph.document(self.node.id).await? {
            self.node.labels.tags = stored.labels.tags;
        }
        for item in &mut self.items {
            item.payload.document_labels = self.node.labels.clone();
        }
        Ok(())
    }
}

async fn ingest_items<E: Embedder, L: Llm>(
    mut document: Document,
    models: &Models<E, L>,
    stores: &Stores,
    on_step: OnStep<'_>,
) -> Result<IngestSummary, IngestError> {
    models
        .concepts
        .check_model_ready()
        .await
        .map_err(IngestError::ModelNotReady)?;
    // It reads the own tags of the stored node, so it must come before an earlier run is removed.
    document.take_labels(stores).await?;
    let Document {
        node,
        items,
        spent,
        chapter,
        ..
    } = document;
    let mut meter = Meter::new(on_step, spent);

    stores.items.ensure_collection().await?;
    stores.concepts.ensure_collection().await?;
    let artifacts = match &chapter {
        Some(chapter) => Some(
            models
                .artifacts
                .write_missing_reporting(&chapter.folder, &mut meter)
                .await?,
        ),
        None => None,
    };
    meter.step(IngestStep::WritingGraph);
    replace_earlier_run(&node, &items, stores).await?;

    let vectors = embed_items(&items, &models.embedder, &mut meter).await?;
    let embedded = EmbeddedItems {
        items: &items,
        vectors: &vectors,
    };
    meter.step(IngestStep::Storing);
    let points_in_collection = store_points(&embedded, &stores.items).await?;
    // The points are stored first, so an insights answer that fails never undoes or repeats them.
    let insights = match (&models.insights, &chapter) {
        (Some(_), Some(chapter)) if !gets_insights(&chapter.name, node.labels.category) => {
            Some(InsightsSummary::none_for_book_part())
        }
        (Some(writer), Some(chapter)) => Some(
            writer
                .write_missing_for_items(&chapter.folder, &items, &mut meter)
                .await,
        ),
        _ => None,
    };
    let concepts = models
        .concepts
        .extract(&embedded, &models.embedder, stores, &mut meter)
        .await?;
    // An item that was skipped was not read for its concepts, so the document is not whole yet.
    // Keep this the last step: a step that failed after it would leave the mark on a run that did
    // not finish.
    if concepts.skipped_items.is_empty() {
        stores
            .graph
            .set_ingested_items(node.id, Some(items.len() as u64))
            .await?;
    }

    Ok(IngestSummary {
        doc_id: node.id,
        doc_title: node.title,
        collection: stores.items.collection().to_owned(),
        items_by_kind: ItemCounts::of(&items),
        points_in_collection,
        concepts,
        artifacts,
        insights,
        usage: meter.into_spent(),
    })
}

/// An earlier run may have cut the document into other items or found other concepts in them, so
/// its points go, then its nodes with their mentions and the mark that it is ingested whole. The
/// points go first, so that no point is ever left without its node.
async fn replace_earlier_run(
    node: &DocumentNode,
    items: &[Item],
    stores: &Stores,
) -> Result<(), IngestError> {
    stores.items.delete_document(node.id).await?;
    // SMELL: from here until the new node is written, the own tags are held by this run only, so a
    // graph that fails in between loses them. The graph cannot remove only the items of a document.
    stores.graph.delete_document(node.id).await?;
    stores.graph.upsert_document(node).await?;
    let nodes: Vec<ItemNode> = items.iter().map(item_node).collect();
    stores.graph.upsert_items(node.id, &nodes).await?;
    Ok(())
}

async fn embed_items<E: Embedder>(
    items: &[Item],
    embedder: &E,
    meter: &mut Meter<'_>,
) -> Result<Vec<Embedding>, IngestError> {
    let inputs: Vec<DocumentInput> = items.iter().map(|item| item.input.clone()).collect();
    meter.step(IngestStep::Embedding { items: items.len() });
    let vectors = embedder.embed_document(&inputs).await?;
    meter.spend(&embedding_usage(&inputs));
    if vectors.len() != items.len() {
        return Err(IngestError::VectorCount {
            items: items.len(),
            vectors: vectors.len(),
        });
    }
    Ok(vectors)
}

/// Stores a point for each item, and returns how many points the whole collection then holds.
async fn store_points(embedded: &EmbeddedItems<'_>, store: &ItemStore) -> Result<u64, StoreError> {
    let points: Vec<ItemPoint> = embedded
        .items
        .iter()
        .zip(embedded.vectors)
        .map(|(item, vector)| ItemPoint {
            id: item.id,
            vector: vector.clone(),
            payload: item.payload.clone(),
        })
        .collect();
    store.upsert(&points).await?;
    store.count().await
}

fn item_node(item: &Item) -> ItemNode {
    ItemNode {
        id: item.id,
        kind: item.payload.kind,
        page: item.payload.page,
        printed_page: item.payload.printed_page.clone(),
    }
}
