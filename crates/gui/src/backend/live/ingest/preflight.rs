//! The free check of a document before a start: what is already converted or ingested, and
//! what would stop a start that can be known without paying.

use graph::GraphStore;
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
    let marked = stores
        .graph
        .ingested_items(document)
        .await
        .map_err(|error| cx.failure(error))?;
    // The collection is asked only for a document that the graph marks, because asking about a
    // collection that does not exist is an error and creating it would write.
    if let Some(items) = marked {
        let stored = stores
            .items
            .count_document(document)
            .await
            .map_err(|error| cx.failure(error))?;
        if stored == items {
            return Ok(Preflight {
                name,
                pages: None,
                state: Some(ChapterState::Ingested { items }),
                blockers: Vec::new(),
            });
        }
    }

    let folder = job.chapter_folder();
    let saved = match ChapterIndex::read(&folder) {
        Ok(saved) => saved,
        Err(ContentError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Preflight {
                name,
                pages: None,
                state: Some(ChapterState::New),
                blockers: Vec::new(),
            });
        }
        Err(error) => return Err(cx.failure(error)),
    };
    if saved.source_sha256 != source_sha256 {
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
    let state = if saved.finished {
        ChapterState::Converted
    } else {
        let mut pages_done = 0;
        for position in 1..=saved.page_count {
            if PageIndex::read(&folder.join(page_folder_name(position))).is_ok() {
                pages_done += 1;
            }
        }
        ChapterState::PartlyConverted { pages_done }
    };
    Ok(Preflight {
        name,
        pages: Some(saved.page_count),
        state: Some(state),
        blockers: Vec::new(),
    })
}
