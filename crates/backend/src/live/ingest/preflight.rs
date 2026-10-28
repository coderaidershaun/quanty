//! The free check of a document before a start: what is already converted or ingested, and
//! what would stop a start that can be known without paying.

use crate::live::convert::IntoView;

use rag_ingestion::{ArtifactState, Blocker, CheckError, DocumentCheck};

use super::chapter_job;
use crate::Reply;
use crate::live::{LiveContext, Services};
use contract::{Event, Failure, IngestRequest, Preflight, RequestId};

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
    let stores = cx.stores().await?;
    let category = Some(ingest.category.into_view());
    let DocumentCheck {
        pages,
        state,
        blockers,
        asks_agent,
    } = rag_ingestion::check_document(&job, &source_sha256, category, &stores)
        .await
        .map_err(|error| match error {
            CheckError::Stores(error) => cx.failure(error),
            CheckError::Folder(error) => cx.failure(error),
        })?;
    let mut blockers: Vec<Failure> = blockers
        .into_iter()
        .map(|blocker| blocker_failure(cx, blocker))
        .collect();
    // The agent is asked last, and only when nothing found so far already stops a start.
    if asks_agent && let Err(failure) = cx.ingest_agent_ready().await {
        blockers.push(failure);
    }
    Ok(Preflight {
        name: ingest.name.clone(),
        pages,
        state: state.map(IntoView::into_view),
        blockers,
    })
}

fn blocker_failure<S: Services>(cx: &LiveContext<S>, blocker: Blocker) -> Failure {
    match blocker {
        Blocker::OtherPdf(error) | Blocker::Unreadable(error) => cx.failure(error),
        Blocker::Metadata { state, reason } => metadata_blocker(state, reason),
    }
}

fn metadata_blocker(state: ArtifactState, reason: String) -> Failure {
    let hint = if state == ArtifactState::NoFolder {
        "Restore the saved converted chapter folder before repairing metadata. Existing searchable data is kept.".to_owned()
    } else {
        reason.clone()
    };
    Failure::new(contract::FailureKind::SourceMissing, reason).with_hint(hint)
}
