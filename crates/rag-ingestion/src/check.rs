//! The free assessment of one document before a start: how far it is converted or ingested, and
//! every reason a start would fail that can be known without paying. The browser and the MCP
//! server both answer their check from it.

use std::path::Path;

use ocr::content::{PageIndex, page_folder_name};
use ocr::{
    ChapterIndex, ChapterInsights, ChapterJob, ContentError, ConvertError, gets_insights,
    pdf_page_count,
};
use rag_core::Category;

use crate::pdf::{PdfError, items_of_ingested_document};
use crate::stores::Stores;
use crate::{ArtifactAssessment, ArtifactState, assess_artifacts};

/// How far a document is on its way to being searchable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentState {
    New,
    PartlyConverted {
        pages_done: u32,
    },
    Converted,
    Ingested {
        items: u64,
    },
    /// The stores hold the document, but its artifact descriptions are not all saved, and its
    /// insights too when `missing_insights` is true.
    IngestedWithoutMetadata {
        items: u64,
        missing_artifacts: bool,
        missing_insights: bool,
    },
    /// The stores hold the document, but a start must write its missing insights.
    IngestedWithoutInsights {
        items: u64,
    },
}

/// A reason a start would fail.
#[derive(Debug)]
pub enum Blocker {
    /// The folder of the document holds another PDF.
    OtherPdf(ConvertError),
    /// The pages of a new PDF cannot be counted, so it cannot be converted either.
    Unreadable(ConvertError),
    /// The required artifact descriptions cannot be completed.
    Metadata {
        state: ArtifactState,
        reason: String,
    },
}

/// What is known about a document without paying for anything or writing anything.
#[derive(Debug)]
pub struct DocumentCheck {
    /// `None` when the pages cannot be counted, and for a document that is stored already.
    pub pages: Option<u32>,
    /// `None` when the folder of the document holds another PDF.
    pub state: Option<DocumentState>,
    /// In the order a start meets them. Empty means a start may go.
    pub blockers: Vec<Blocker>,
    /// True when a start would ask the ingest agent, so the caller checks that it is ready and
    /// adds what it finds to the blockers, last.
    pub asks_agent: bool,
}

/// Why a check could not say what the stores and the folder hold.
#[derive(thiserror::Error, Debug)]
pub enum CheckError {
    #[error("could not ask the stores whether the document is ingested")]
    Stores(#[source] PdfError),

    #[error("could not read the folder of the document")]
    Folder(#[source] ContentError),
}

/// Assesses the document of `job`, whose PDF has the SHA-256 `source_sha256`. `category` is the
/// category of its media, which decides whether it gets insights.
///
/// # Errors
/// - [`CheckError::Stores`] when a store cannot say what it holds
/// - [`CheckError::Folder`] when the folder of the document cannot be read for a reason other than
///   not being there
pub async fn check_document(
    job: &ChapterJob,
    source_sha256: &str,
    category: Option<Category>,
    stores: &Stores,
) -> Result<DocumentCheck, CheckError> {
    let document = rag_core::DocId::from_source_sha256(source_sha256);
    let ingested = items_of_ingested_document(document, stores)
        .await
        .map_err(CheckError::Stores)?;
    match ingested {
        Some(items) => Ok(ingested_check(job, category, items)),
        None => folder_check(job, source_sha256).await,
    }
}

/// A stored document needs the agent only when artifact descriptions or enabled insights need
/// repair.
fn ingested_check(job: &ChapterJob, category: Option<Category>, items: u64) -> DocumentCheck {
    let folder = job.chapter_folder();
    // A file that cannot be read counts as none, as it does when the insights are written.
    let missing_insights = gets_insights(job.name(), category)
        && !matches!(ChapterInsights::read(&folder), Ok(Some(_)));
    let assessment = assess_artifacts(&folder);
    let missing_artifacts = assessment.state != ArtifactState::Ready;
    let blockers: Vec<Blocker> = metadata_blocker(assessment).into_iter().collect();
    let (state, asks_agent) = if missing_artifacts {
        let state = DocumentState::IngestedWithoutMetadata {
            items,
            missing_artifacts,
            missing_insights,
        };
        (state, blockers.is_empty())
    } else if missing_insights {
        (DocumentState::IngestedWithoutInsights { items }, true)
    } else {
        (DocumentState::Ingested { items }, false)
    };
    DocumentCheck {
        pages: None,
        state: Some(state),
        blockers,
        asks_agent,
    }
}

/// What the folder of the document says: how far it is converted, or, when it holds another PDF,
/// no state and the blocker that says so.
async fn folder_check(job: &ChapterJob, source_sha256: &str) -> Result<DocumentCheck, CheckError> {
    let folder = job.chapter_folder();
    let mut blockers = Vec::new();
    let (pages, state) = match ChapterIndex::read(&folder) {
        Ok(saved) if saved.source_sha256 != source_sha256 => {
            let other_pdf = ConvertError::DifferentSource {
                folder,
                saved_file: saved.source_file,
                given_file: ocr::content::file_name_text(job.chapter_pdf()),
            };
            return Ok(DocumentCheck {
                pages: None,
                state: None,
                blockers: vec![Blocker::OtherPdf(other_pdf)],
                asks_agent: false,
            });
        }
        Ok(saved) => (Some(saved.page_count), converted_so_far(&folder, &saved)),
        Err(ContentError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            match pdf_page_count(job.chapter_pdf()).await {
                Ok(pages) => (Some(pages), DocumentState::New),
                // A PDF whose pages cannot be counted cannot be converted either.
                Err(error) => {
                    blockers.push(Blocker::Unreadable(ConvertError::from(error)));
                    (None, DocumentState::New)
                }
            }
        }
        Err(error) => return Err(CheckError::Folder(error)),
    };
    if state == DocumentState::Converted {
        blockers.extend(metadata_blocker(assess_artifacts(&folder)));
    }
    Ok(DocumentCheck {
        pages,
        state: Some(state),
        blockers,
        asks_agent: true,
    })
}

fn converted_so_far(folder: &Path, saved: &ChapterIndex) -> DocumentState {
    if saved.finished {
        return DocumentState::Converted;
    }
    let mut pages_done = 0;
    for position in 1..=saved.page_count {
        if PageIndex::read(&folder.join(page_folder_name(position))).is_ok() {
            pages_done += 1;
        }
    }
    DocumentState::PartlyConverted { pages_done }
}

fn metadata_blocker(assessment: ArtifactAssessment) -> Option<Blocker> {
    matches!(
        assessment.state,
        ArtifactState::NoFolder | ArtifactState::TooLarge
    )
    .then(|| Blocker::Metadata {
        state: assessment.state,
        reason: assessment
            .reason
            .unwrap_or_else(|| "required artifact metadata cannot be completed".to_owned()),
    })
}
