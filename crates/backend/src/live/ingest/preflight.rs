//! The free check of a document before a start: what is already converted or ingested, and
//! what would stop a start that can be known without paying.

use crate::live::convert::IntoView;

use std::path::Path;

use ocr::content::{PageIndex, page_folder_name};
use ocr::{ChapterIndex, ChapterInsights, ChapterJob, ContentError, ConvertError, gets_insights};
use rag_ingestion::ArtifactState;

use super::{chapter_job, uploaded_source};
use crate::Reply;
use crate::live::{LiveContext, Services};
use contract::{ChapterState, Event, Failure, IngestRequest, Preflight, RequestId};

/// It pays for nothing, writes to no store and makes no folder.
pub async fn preflight<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    ingest: &IngestRequest,
    reply: &Reply,
) {
    let result = check(cx, ingest).await;
    reply.send(Event::Preflight { request, result });
}

async fn check<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<Preflight, Failure> {
    let job = chapter_job(cx, ingest)?;
    let source_sha256 = job.source_sha256().map_err(|error| cx.failure(error))?;
    let document = rag_core::DocId::from_source_sha256(&source_sha256);
    let stores = cx.stores().await?;
    let ingested = rag_ingestion::items_of_ingested_document(document, &stores)
        .await
        .map_err(|error| cx.failure(error))?;
    if let Some(items) = ingested {
        return Ok(ingested_check(cx, ingest, &job, items).await);
    }
    let mut preflight = folder_check(cx, ingest, &job, &source_sha256).await?;
    // The agent is asked last, and only when the folder said nothing that already stops a start.
    if preflight.state.is_some()
        && let Err(failure) = cx.ingest_agent_ready().await
    {
        preflight.blockers.push(failure);
    }
    Ok(preflight)
}

/// Stored documents need the agent only when artifact descriptions or enabled insights need repair.
async fn ingested_check<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
    job: &ChapterJob,
    items: u64,
) -> Preflight {
    let gets = gets_insights(job.name(), Some(ingest.category.into_view()));
    // A file that cannot be read counts as none, as it does when the insights are written.
    let missing = gets && !matches!(ChapterInsights::read(&job.chapter_folder()), Ok(Some(_)));
    let (artifacts, blocker) = assess_artifacts(&job.chapter_folder());
    let missing_artifacts = artifacts != ArtifactState::Ready;
    let mut blockers: Vec<Failure> = blocker.into_iter().collect();
    let state = if missing_artifacts {
        if blockers.is_empty()
            && let Err(failure) = cx.ingest_agent_ready().await
        {
            blockers.push(failure);
        }
        ChapterState::IngestedWithoutMetadata {
            items,
            missing_artifacts,
            missing_insights: missing,
        }
    } else if missing {
        if let Err(failure) = cx.ingest_agent_ready().await {
            blockers.push(failure);
        }
        ChapterState::IngestedWithoutInsights { items }
    } else {
        ChapterState::Ingested { items }
    };
    Preflight {
        name: ingest.name.clone(),
        pages: None,
        state: Some(state),
        blockers,
    }
}

/// What the folder of the document says: how far it is converted, or, when it holds another
/// PDF, no state and the blocker that says so.
async fn folder_check<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
    job: &ChapterJob,
    source_sha256: &str,
) -> Result<Preflight, Failure> {
    let name = ingest.name.clone();
    let folder = job.chapter_folder();
    let mut blockers = Vec::new();
    let (pages, state) = match ChapterIndex::read(&folder) {
        Ok(saved) if saved.source_sha256 != source_sha256 => {
            let taken = ConvertError::DifferentSource {
                folder,
                saved_file: saved.source_file,
                given_file: ingest.file_name.clone(),
            };
            return Ok(Preflight {
                name,
                pages: None,
                state: None,
                blockers: vec![cx.failure(taken)],
            });
        }
        Ok(saved) => (Some(saved.page_count), converted_so_far(&folder, &saved)),
        Err(ContentError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            match ocr::pdf_page_count(&uploaded_source(cx, ingest)?).await {
                Ok(pages) => (Some(pages), ChapterState::New),
                // A PDF whose pages cannot be counted cannot be converted either.
                Err(error) => {
                    blockers.push(cx.failure(ConvertError::from(error)));
                    (None, ChapterState::New)
                }
            }
        }
        Err(error) => return Err(cx.failure(error)),
    };
    if state == ChapterState::Converted {
        blockers.extend(assess_artifacts(&folder).1);
    }
    Ok(Preflight {
        name,
        pages,
        state: Some(state),
        blockers,
    })
}

fn converted_so_far(folder: &Path, saved: &ChapterIndex) -> ChapterState {
    if saved.finished {
        return ChapterState::Converted;
    }
    let mut pages_done = 0;
    for position in 1..=saved.page_count {
        if PageIndex::read(&folder.join(page_folder_name(position))).is_ok() {
            pages_done += 1;
        }
    }
    ChapterState::PartlyConverted { pages_done }
}

/// The state of the artifact metadata of a chapter, and the blocker when it cannot be completed.
fn assess_artifacts(folder: &Path) -> (ArtifactState, Option<Failure>) {
    let assessment = rag_ingestion::assess_artifacts(folder);
    let state = assessment.state;
    let blocked = matches!(state, ArtifactState::NoFolder | ArtifactState::TooLarge)
        .then(|| metadata_blocker(assessment));
    (state, blocked)
}

fn metadata_blocker(assessment: rag_ingestion::ArtifactAssessment) -> Failure {
    let reason = assessment
        .reason
        .unwrap_or_else(|| "required artifact metadata cannot be completed".to_owned());
    let hint = if assessment.state == ArtifactState::NoFolder {
        "Restore the saved converted chapter folder before repairing metadata. Existing searchable data is kept.".to_owned()
    } else {
        reason.clone()
    };
    Failure::new(contract::FailureKind::SourceMissing, reason).with_hint(hint)
}
