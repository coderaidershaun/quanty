//! Takes one PDF all the way into the stores in one run: name it by the category of its media,
//! convert its pages, then ingest the converted document. A PDF that is already ingested is not
//! touched.

use std::fmt;
use std::path::Path;

use graph::{GraphError, GraphStore};
use ocr::content::parse_chapter_file_name;
use ocr::{
    ChapterJob, ContentError, ConversionSummary, ConvertError, DocumentName, MediaDocument,
    PageProgress,
};
use rag_core::{Category, DocId, Embedder, Llm, MediaLabels, StoreError, UsageTally};

use crate::ingest::{
    ChapterFolder, IngestError, IngestStep, IngestSummary, Models, ingest_chapter_reporting,
    usage_of, write_usage,
};
use crate::media::media_category;
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

/// What a PDF is named from, before the graph says which category its media has.
#[derive(Debug, Clone, Copy)]
pub struct UnnamedPdf<'a> {
    pub media_title: &'a str,
    /// Makes a new media only: a media the library has keeps its own category.
    pub category: Option<Category>,
    /// The document's own title, for a paper or another media. A blank one counts as not given.
    pub document_title: Option<&'a str>,
    pub pdf: &'a Path,
}

/// A PDF with the name of its document, and the category that named it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPdf {
    /// The category of the stored media when the library has it, else the one given, else a book.
    /// A new media is made with it.
    pub category: Category,
    pub document: MediaDocument,
}

#[derive(thiserror::Error, Debug)]
pub enum NamePdfError {
    #[error("could not read from the graph which media the library has")]
    ReadMedia(#[source] GraphError),

    #[error("a book's chapter is named by its file name, so it takes no document title")]
    TitleForABook,

    #[error("the pdf of a book must be named chapter-<number>-<name>.pdf")]
    ChapterFileName(#[source] ContentError),
}

/// One chapter PDF and the way its pages get converted: the real services in the command,
/// stand-ins in a test.
pub struct ChapterPdf<'a, C, O> {
    pub job: &'a ChapterJob,
    /// The labels the media is made with when the graph does not have it yet.
    pub new_media: &'a MediaLabels,
    /// Called at most once, and only when the document is not ingested yet. Its second argument
    /// hears the count of the pages, then each page as it is cut out and as it ends.
    pub convert: C,
    /// Hears each step of the run as it happens, with what the run used up to and with that step.
    /// A PDF that is ingested already hears only the check of the stores.
    pub on_step: O,
}

impl PdfOutcome {
    pub fn doc_id(&self) -> DocId {
        match self {
            PdfOutcome::AlreadyIngested { doc_id, .. } => *doc_id,
            PdfOutcome::Ingested(summary) => summary.ingest.doc_id,
        }
    }
}

impl PdfSummary {
    /// What the whole run used: the conversion of the pages that this run converted, then the
    /// ingest.
    pub fn usage(&self) -> UsageTally {
        let mut usage = usage_of(&self.conversion.calls);
        usage.add_all(&self.ingest.usage);
        usage
    }
}

/// The conversion's own lines of tokens are left out, and the lines of the whole run come after the
/// counts, so a model that both parts used has one line, under the name of the table of prices.
impl fmt::Display for PdfSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut conversion = self.conversion.clone();
        conversion.calls.by_model.clear();
        writeln!(formatter, "{conversion}")?;
        self.ingest.write_counts(formatter)?;
        let usage = self.usage();
        write_usage(formatter, &usage, usage.cost_usd())
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
/// with `pdf.convert` and ingests the converted folder with [`crate::ingest_chapter`].
///
/// Each step is told to `pdf.on_step` as it happens: the check of the stores first, the opening
/// of the PDF just before the conversion, then each page and each later step. With each step
/// comes what the run used so far: the pages converted so far, then the whole conversion and what
/// the ingest used.
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
pub async fn ingest_pdf<C, O, E: Embedder, L: Llm, G: GraphStore>(
    pdf: ChapterPdf<'_, C, O>,
    models: &Models<E, L>,
    stores: &Stores<G>,
) -> Result<PdfOutcome, PdfError>
where
    C: AsyncFnOnce(
        &ChapterJob,
        &mut (dyn FnMut(PageProgress) + Send),
    ) -> Result<ConversionSummary, ConvertError>,
    O: FnMut(IngestStep, &UsageTally) + Send,
{
    let ChapterPdf {
        job,
        new_media,
        convert,
        mut on_step,
    } = pdf;
    on_step(IngestStep::CheckingStored, &UsageTally::default());
    let document = DocId::from_source_sha256(&job.source_sha256()?);
    if let Some(items) = already_ingested(document, stores).await? {
        return Ok(PdfOutcome::AlreadyIngested {
            doc_id: document,
            items,
        });
    }
    on_step(IngestStep::OpeningPdf, &UsageTally::default());
    let mut pages_so_far = UsageTally::default();
    let conversion = convert(job, &mut |page| {
        if let PageProgress::PageDone { calls, .. } = &page {
            pages_so_far.add_all(&usage_of(calls));
        }
        on_step(IngestStep::Converting(page), &pages_so_far);
    })
    .await?;
    let chapter = ChapterFolder {
        folder: &job.chapter_folder(),
        new_media,
    };
    let converted = usage_of(&conversion.calls);
    let ingest = ingest_chapter_reporting(chapter, models, stores, &mut |step, spent| {
        let mut so_far = converted.clone();
        so_far.add_all(spent);
        on_step(step, &so_far);
    })
    .await?;
    Ok(PdfOutcome::Ingested(Box::new(PdfSummary {
        conversion,
        ingest,
    })))
}

impl UnnamedPdf<'_> {
    /// Names the document by the category of its media, which the graph is asked for. A book's
    /// PDF must be named `chapter-<number>-<name>.pdf` and takes no document title. A paper or
    /// another media takes the document's own title, or else the media's. It writes nothing.
    ///
    /// # Errors
    /// - [`NamePdfError::ReadMedia`] when the graph cannot list its media
    /// - [`NamePdfError::TitleForABook`] when a document title was given for a book's PDF
    /// - [`NamePdfError::ChapterFileName`] when a book's PDF is named in another way
    pub async fn named<G: GraphStore>(&self, graph: &G) -> Result<NamedPdf, NamePdfError> {
        let category = media_category(self.media_title, self.category, graph)
            .await
            .map_err(NamePdfError::ReadMedia)?;
        let document_title = self
            .document_title
            .map(str::trim)
            .filter(|title| !title.is_empty());
        let name = match (category, document_title) {
            (Category::Book, Some(_)) => return Err(NamePdfError::TitleForABook),
            (Category::Book, None) => {
                let file_name = self
                    .pdf
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                parse_chapter_file_name(&file_name).map_err(NamePdfError::ChapterFileName)?
            }
            (Category::Paper | Category::Other, title) => {
                DocumentName::Title(title.unwrap_or(self.media_title).trim().to_owned())
            }
        };
        Ok(NamedPdf {
            category,
            document: MediaDocument {
                media_title: self.media_title.to_owned(),
                name,
            },
        })
    }
}

/// [`items_of_ingested_document`], with the collection made first: that fails while Qdrant is
/// down, before a page is paid for, and a collection that was removed then counts as holding no
/// point.
async fn already_ingested<G: GraphStore>(
    document: DocId,
    stores: &Stores<G>,
) -> Result<Option<u64>, PdfError> {
    stores.items.ensure_collection().await?;
    items_of_ingested_document(document, stores).await
}

/// The number of items the document was ingested whole with, when the graph says so and the
/// collection holds exactly that many points of it, and `None` in every other case. It writes
/// nothing, so a check before a start may call it.
///
/// # Errors
/// [`PdfError::Graph`] and [`PdfError::Store`] when a store cannot say what it holds.
// SMELL: only the stores are asked, so a document whose converted folder was removed after the
// ingest still counts as ingested, and the pictures its items name are gone. It stays because the
// free check of the desktop app reads the same answer, and converting it again would be paid.
pub async fn items_of_ingested_document<G: GraphStore>(
    document: DocId,
    stores: &Stores<G>,
) -> Result<Option<u64>, PdfError> {
    let Some(marked) = stores.graph.ingested_items(document).await? else {
        return Ok(None);
    };
    let stored = stores.items.count_document(document).await?;
    Ok((stored == marked).then_some(marked))
}
