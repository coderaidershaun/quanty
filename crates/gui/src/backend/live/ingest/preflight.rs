//! The free check of a document before a start: what is already converted or ingested, and
//! what would stop a start that can be known without paying.

use std::path::Path;

use ocr::content::{PageIndex, page_folder_name};
use ocr::{ChapterIndex, ContentError, ConvertError};

use super::{chapter_job, file_name_of};
use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{ChapterState, Event, Failure, IngestRequest, Preflight, RequestId};

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
    let name = ingest.name.clone();
    let source_sha256 = job.source_sha256().map_err(|error| cx.failure(error))?;
    let document = rag_core::DocId::from_source_sha256(&source_sha256);
    let stores = cx.stores().await?;
    let ingested = rag_ingestion::items_of_ingested_document(document, &stores)
        .await
        .map_err(|error| cx.failure(error))?;
    // A start of an ingested document pays for nothing, so nothing can block it.
    if let Some(items) = ingested {
        return Ok(Preflight {
            name,
            pages: None,
            state: Some(ChapterState::Ingested { items }),
            blockers: Vec::new(),
        });
    }

    let folder = job.chapter_folder();
    let mut blockers = Vec::new();
    let (pages, state) = match ChapterIndex::read(&folder) {
        Ok(saved) if saved.source_sha256 != source_sha256 => {
            let taken = ConvertError::DifferentSource {
                folder,
                saved_file: saved.source_file,
                given_file: file_name_of(&ingest.pdf),
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
            match ocr::pdf_page_count(&ingest.pdf).await {
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
    // `claude` is asked last, because a document that is ingested already, or whose folder holds
    // another PDF, needs no answer from it.
    if let Err(failure) = cx.claude_ready().await {
        blockers.push(failure);
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
