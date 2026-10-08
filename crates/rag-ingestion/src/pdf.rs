//! Takes one chapter PDF all the way into the stores in one run: convert its pages, then ingest
//! the converted chapter. A PDF that is already ingested is not touched.

use std::fmt;
use std::path::Path;

use graph::{GraphError, GraphStore};
use ocr::content::parse_chapter_file_name;
use ocr::{ChapterJob, ContentError, ConversionSummary, ConvertError, DocumentName};
use rag_core::{Category, DocId, Embedder, Llm, MediaLabels, StoreError};

use crate::ingest::{ChapterFolder, IngestError, IngestSummary, Models, ingest_chapter};
use crate::stores::Stores;

#[derive(Debug, Clone, PartialEq)]
pub enum PdfOutcome {
    /// Both stores already hold the whole document, so nothing was converted, embedded, asked or
    /// written.
    AlreadyIngested { doc_id: DocId, items: u64 },
    /// The chapter was converted, or was found converted, and then ingested.
    Ingested(Box<PdfSummary>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfSummary {
    pub conversion: ConversionSummary,
    pub ingest: IngestSummary,
}

#[derive(thiserror::Error, Debug)]
pub enum PdfError {
    #[error("could not convert the chapter pdf")]
    Convert(#[from] ConvertError),

    #[error("could not read from the graph whether the document is already ingested")]
    Graph(#[from] GraphError),

    #[error("could not check the items collection for the document")]
    Store(#[from] StoreError),

    #[error("could not ingest the converted chapter")]
    Ingest(#[from] IngestError),
}

/// One chapter PDF and the way its pages get converted: the real services in the command,
/// stand-ins in a test.
pub struct ChapterPdf<'a, C> {
    pub job: &'a ChapterJob,
    /// The labels the media is made with when the graph does not have it yet.
    pub new_media: &'a MediaLabels,
    /// Called at most once, and only when the document is not ingested yet.
    pub convert: C,
}

impl PdfOutcome {
    pub fn doc_id(&self) -> DocId {
        match self {
            PdfOutcome::AlreadyIngested { doc_id, .. } => *doc_id,
            PdfOutcome::Ingested(summary) => summary.ingest.doc_id,
        }
    }
}

impl fmt::Display for PdfSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "{}", self.conversion)?;
        write!(formatter, "{}", self.ingest)
    }
}

impl fmt::Display for PdfOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfOutcome::AlreadyIngested { doc_id, items } => {
                writeln!(formatter, "document id: {doc_id}")?;
                write!(
                    formatter,
                    "already ingested: both stores hold all {items} items of this document, so nothing was converted, embedded, asked or written"
                )
            }
            PdfOutcome::Ingested(summary) => summary.fmt(formatter),
        }
    }
}

/// Hashes the PDF, and unless both stores already hold the whole document, converts the chapter
/// with `pdf.convert` and ingests the converted folder with [`ingest_chapter`].
///
/// Both stores are checked before the first page is converted, so a store that is down fails the
/// run before a page is paid for. A second call on the same PDF converts, embeds, asks and
/// writes nothing. A call that stopped in the conversion, or in the ingest, is finished by the
/// same call again: the pages that were converted are kept, and an ingest writes the same points
/// and nodes over themselves. A document that was ingested with an item that was skipped is not
/// taken as ingested, so the next call asks about that item again.
///
/// # Errors
/// - [`PdfError::Convert`] when the PDF cannot be read or converted; a page that fails stops the
///   run and its number is in the error
/// - [`PdfError::Graph`] and [`PdfError::Store`] when a store cannot say what it holds
/// - [`PdfError::Ingest`] when the ingest fails
pub async fn ingest_pdf<C, E: Embedder, L: Llm, G: GraphStore>(
    pdf: ChapterPdf<'_, C>,
    models: &Models<E, L>,
    stores: &Stores<G>,
) -> Result<PdfOutcome, PdfError>
where
    C: AsyncFnOnce(&ChapterJob) -> Result<ConversionSummary, ConvertError>,
{
    let document = DocId::from_source_sha256(&pdf.job.source_sha256()?);
    // This comes first: it fails while Qdrant is down, before a page is paid for, and a
    // collection that was removed then counts as holding no point.
    stores.items.ensure_collection().await?;
    // SMELL: only the stores are asked, and the content folder is never looked at. A document
    // whose converted chapter folder was removed or moved after the ingest still counts as
    // ingested, and the picture paths that its items keep then point at nothing.
    if let Some(items) = items_of_ingested_document(document, stores).await? {
        return Ok(PdfOutcome::AlreadyIngested {
            doc_id: document,
            items,
        });
    }
    let conversion = (pdf.convert)(pdf.job).await?;
    let chapter = ChapterFolder {
        folder: &pdf.job.chapter_folder(),
        new_media: pdf.new_media,
    };
    let ingest = ingest_chapter(chapter, models, stores).await?;
    Ok(PdfOutcome::Ingested(Box::new(PdfSummary {
        conversion,
        ingest,
    })))
}

/// How a PDF of a media of that category is named. A book's PDF must be named
/// `chapter-<number>-<name>.pdf`, and `title` is not used. A paper or another media takes `title`:
/// the caller gives the document's own title when there is one, and else the media's title.
///
/// # Errors
/// [`ContentError::BadFileName`] when the PDF of a book is named in another way.
// SMELL: the caller gives the category, and it can differ from the category of a media that the
// library already has. The document is then named by one category and labelled with the other,
// and nothing checks that the two agree.
pub fn document_name(
    category: Category,
    title: &str,
    pdf: &Path,
) -> Result<DocumentName, ContentError> {
    match category {
        Category::Book => {
            let file_name = pdf
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            parse_chapter_file_name(&file_name)
        }
        Category::Paper | Category::Other => Ok(DocumentName::Title(title.trim().to_owned())),
    }
}

/// The number of items the document was ingested whole with, when the graph says so and the
/// collection holds exactly that many points of it, and `None` in every other case.
async fn items_of_ingested_document<G: GraphStore>(
    document: DocId,
    stores: &Stores<G>,
) -> Result<Option<u64>, PdfError> {
    let Some(marked) = stores.graph.ingested_items(document).await? else {
        return Ok(None);
    };
    let stored = stores.items.count_document(document).await?;
    Ok((stored == marked).then_some(marked))
}
