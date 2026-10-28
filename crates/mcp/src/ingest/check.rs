//! The `check_pdf` tool: what an `ingest_pdf` of the same PDF would do, found without a job, a
//! paid call or a write to the stores or to the content folder.

use graph::FalkorGraph;
use ocr::convert::services::{COPY_MODEL, TAG_MODEL, TRANSCRIBE_MODEL};
use rag_core::{
    ConceptStore, DocId, ItemStore, Llm, Usage, WorkKind, error_chain, one_line, price_of,
};
use rag_ingestion::{Blocker, DocumentState, Stores, check_document, document_title_of};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Ingest, IngestPdfArgs, PdfIngestError, pdf};
use crate::services::Services;

/// Seconds to convert one page, measured on the committed sample chapter.
const SECONDS_PER_PAGE: u32 = 28;

// The tokens of the three calls that convert a page are rough figures, read from the page
// records of a sample chapter. A page that is long or dense uses more.
const TAG_USE: Usage = tokens(13_000, 2_000);
const COPY_USE: Usage = tokens(16_000, 8_000);
const TRANSCRIBE_USE: Usage = tokens(25_000, 2_000);

const fn tokens(input_tokens: u64, output_tokens: u64) -> Usage {
    Usage {
        input_tokens,
        output_tokens,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    }
}

/// What an agent gives `check_pdf`: the same PDF arguments as `ingest_pdf`, without the labels
/// and without `pdf_base64`. Give exactly one of `path` and `pdf_url`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct CheckPdfArgs {
    /// The title of the media the PDF belongs to, such as "Option Volatility and Pricing".
    // An agent that still sends the older name `book` is understood. The schema shows `media`
    // only.
    #[serde(alias = "book")]
    media: String,
    /// `book` (the default), `paper` or `other`. Used only when the library does not have this
    /// media yet: a stored media keeps its own category.
    category: Option<String>,
    /// The title of this document, as `ingest_pdf` takes it.
    document_title: Option<String>,
    /// The absolute path of the PDF on the machine the server runs on.
    path: Option<String>,
    /// An http or https URL that the server fetches the PDF from, as `ingest_pdf` takes it. It
    /// goes with `file_name`. The PDF is held in a temporary file that is removed when the check
    /// ends.
    pdf_url: Option<String>,
    /// The name of the PDF fetched from `pdf_url`, as `ingest_pdf` takes it. Not for `path`.
    file_name: Option<String>,
}

/// Lets `check_pdf` run the checks of `ingest_pdf` on the same PDF arguments, with no labels and
/// no base64.
impl From<CheckPdfArgs> for IngestPdfArgs {
    fn from(args: CheckPdfArgs) -> IngestPdfArgs {
        IngestPdfArgs {
            media: args.media,
            category: args.category,
            authors: None,
            tags: None,
            document_tags: None,
            document_title: args.document_title,
            path: args.path,
            pdf_base64: None,
            pdf_url: args.pdf_url,
            file_name: args.file_name,
        }
    }
}

/// How far the PDF is on its way to being searchable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum PdfState {
    /// Nothing of it is converted or stored.
    New,
    /// Some pages are converted; `pages_done` says how many.
    PartlyConverted,
    /// Every page is converted, and the stores do not hold it yet.
    Converted,
    /// The stores hold it whole; `items` says how many.
    Ingested,
    /// The stores hold it, but a start would write its missing insights.
    IngestedWithoutInsights,
    /// The stores hold it, but a start would repair its artifact descriptions, and its insights
    /// too when `missing_insights` is true.
    IngestedWithoutMetadata,
}

/// The cost and time of converting the pages, the part of an ingest that is paid for each page. It
/// uses the API prices of the price table of quanty. The later steps (artifact descriptions,
/// concepts, embedding and insights) cost extra.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
struct Estimate {
    /// The pages that a start would convert: all of a new PDF, the rest of a partly converted
    /// one, none of the others.
    pages_to_convert: u32,
    /// US dollars when every page is a plain page, rounded up to the cent. `null` when the price
    /// table has no price for a model.
    usd_low: Option<f64>,
    /// US dollars when every page is read as math or figures, rounded up to the cent. `null` when
    /// the price table has no price for a model.
    usd_high: Option<f64>,
    /// Minutes to convert the pages, rounded up.
    minutes: u32,
}

/// What an `ingest_pdf` of the PDF would do. It starts no job and writes nothing.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PdfCheck {
    /// The media title, as given.
    media: String,
    /// The title the document has once it is ingested, as `list_documents` shows it.
    title: String,
    /// The category that names the PDF: the stored media's, else the one given, else `book`.
    category: String,
    /// The file name.
    file: String,
    /// The id the document has once it is ingested.
    document_id: String,
    /// The pages of the PDF. `null` when they cannot be counted (a blocker says why) and for a
    /// document that is stored already.
    pages: Option<u32>,
    /// `null` when the folder of the document holds a different PDF (a blocker says so).
    state: Option<PdfState>,
    /// Only for `partly_converted`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pages_done: Option<u32>,
    /// Only for the `ingested` states.
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<u64>,
    /// Only for `ingested_without_metadata`.
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_artifacts: Option<bool>,
    /// Only for `ingested_without_metadata`.
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_insights: Option<bool>,
    /// Every reason an `ingest_pdf` would fail, one line each. Empty means it may start.
    blockers: Vec<String>,
    estimate: Estimate,
}

impl<S: Services> Ingest<S> {
    /// Checks the PDF as `ingest_pdf` does and says what a start would do. A PDF named by URL is
    /// fetched into a temporary file that is removed before this returns.
    ///
    /// # Errors
    /// A PDF that is refused, a store that cannot be reached or cannot say what it holds, or a PDF
    /// that cannot be read.
    pub(crate) async fn check(&self, args: CheckPdfArgs) -> Result<PdfCheck, PdfIngestError> {
        let mut checked = pdf::check(args.into(), &self.config, self.max_pdf_bytes).await?;
        // The temporary folder is held to the end, so that a fetched PDF is still there while
        // it is read.
        let _fetched = checked.keep_upload_in_a_temporary_folder().await?;
        let graph = FalkorGraph::connect(&self.config).await?;
        let ready = checked.named(&graph, &self.config).await?;
        let job = ready.chapter.clone();
        let source_sha256 = tokio::task::spawn_blocking(move || job.source_sha256())
            .await?
            .map_err(PdfIngestError::Source)?;
        let stores = Stores {
            items: ItemStore::connect(&self.config).map_err(PdfIngestError::ItemStore)?,
            graph,
            concepts: ConceptStore::connect(&self.config).map_err(PdfIngestError::ConceptStore)?,
        };
        let category = ready.new_media.category;
        let document = check_document(&ready.chapter, &source_sha256, Some(category), &stores)
            .await
            .map_err(|error| PdfIngestError::Check(Box::new(error)))?;
        let mut blockers: Vec<String> = document.blockers.iter().map(blocker_text).collect();
        // The agent is asked last, and only when nothing found so far already stops a start.
        if document.asks_agent
            && let Err(error) = self.services.llm(WorkKind::Ingest).check_ready().await
        {
            blockers.push(one_line(&error_chain(&error)));
        }
        let mut check = PdfCheck {
            title: document_title_of(&ready.media, ready.chapter.name()),
            media: ready.media,
            category: category.to_string(),
            file: ready.file_name,
            document_id: DocId::from_source_sha256(&source_sha256).to_string(),
            pages: document.pages,
            state: None,
            pages_done: None,
            items: None,
            missing_artifacts: None,
            missing_insights: None,
            blockers,
            estimate: Estimate::of(pages_to_convert(document.pages, document.state)),
        };
        check.set_state(document.state);
        Ok(check)
    }
}

fn blocker_text(blocker: &Blocker) -> String {
    match blocker {
        Blocker::OtherPdf(error) | Blocker::Unreadable(error) => one_line(&error_chain(error)),
        Blocker::Metadata { reason, .. } => one_line(reason),
    }
}

/// A new PDF converts all its pages, a partly converted one the rest, and any other none.
fn pages_to_convert(pages: Option<u32>, state: Option<DocumentState>) -> u32 {
    let pages = pages.unwrap_or(0);
    match state {
        Some(DocumentState::New) => pages,
        Some(DocumentState::PartlyConverted { pages_done }) => pages.saturating_sub(pages_done),
        _ => 0,
    }
}

impl PdfCheck {
    /// Sets the state and the fields that only that state has.
    fn set_state(&mut self, state: Option<DocumentState>) {
        match state {
            None => {}
            Some(DocumentState::New) => self.state = Some(PdfState::New),
            Some(DocumentState::PartlyConverted { pages_done }) => {
                self.state = Some(PdfState::PartlyConverted);
                self.pages_done = Some(pages_done);
            }
            Some(DocumentState::Converted) => self.state = Some(PdfState::Converted),
            Some(DocumentState::Ingested { items }) => {
                self.state = Some(PdfState::Ingested);
                self.items = Some(items);
            }
            Some(DocumentState::IngestedWithoutInsights { items }) => {
                self.state = Some(PdfState::IngestedWithoutInsights);
                self.items = Some(items);
            }
            Some(DocumentState::IngestedWithoutMetadata {
                items,
                missing_artifacts,
                missing_insights,
            }) => {
                self.state = Some(PdfState::IngestedWithoutMetadata);
                self.items = Some(items);
                self.missing_artifacts = Some(missing_artifacts);
                self.missing_insights = Some(missing_insights);
            }
        }
    }
}

impl Estimate {
    /// The estimate for `pages_to_convert` pages. The low figure reads every page as plain text
    /// and the high one reads every page as math or figures; each is tagged first.
    fn of(pages_to_convert: u32) -> Estimate {
        let pages = f64::from(pages_to_convert);
        let cost = |calls: [(&str, Usage); 2]| -> Option<f64> {
            let per_page = calls
                .iter()
                .map(|(model, usage)| price_of(model).map(|price| price.cost_usd(usage)))
                .sum::<Option<f64>>()?;
            Some(rounded_up_to_the_cent(pages * per_page))
        };
        Estimate {
            pages_to_convert,
            usd_low: cost([(TAG_MODEL, TAG_USE), (COPY_MODEL, COPY_USE)]),
            usd_high: cost([(TAG_MODEL, TAG_USE), (TRANSCRIBE_MODEL, TRANSCRIBE_USE)]),
            minutes: (pages_to_convert * SECONDS_PER_PAGE).div_ceil(60),
        }
    }
}

fn rounded_up_to_the_cent(usd: f64) -> f64 {
    // Whole millionths first, so that float noise just above a whole cent does not add a cent.
    let millionths = (usd * 1_000_000.0).round();
    (millionths / 10_000.0).ceil() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_whole_cents(usd: f64) -> bool {
        (usd * 100.0 - (usd * 100.0).round()).abs() < 1e-9
    }

    #[test]
    fn the_estimate_rounds_up_to_the_cent_and_takes_28_seconds_a_page() {
        let none = Estimate::of(0);
        assert_eq!(none.minutes, 0);
        assert_eq!((none.usd_low, none.usd_high), (Some(0.0), Some(0.0)));

        let seven = Estimate::of(7);

        assert_eq!(seven.minutes, 4);
        let (low, high) = (seven.usd_low.unwrap(), seven.usd_high.unwrap());
        assert!(0.0 < low && low < high, "{low} {high}");
        assert!(is_whole_cents(low) && is_whole_cents(high), "{low} {high}");
    }
}
