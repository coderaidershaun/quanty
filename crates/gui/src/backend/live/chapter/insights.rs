//! Reads the saved insights of a stored document from its chapter folder on disk, so the Library
//! shows them while the stores are down.

use std::path::{Path, PathBuf};

use super::folder::chapters_on_disk;
use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{ChapterInsights, DocId, Event, Failure, Insight, RequestId};

/// Sends one `Event::ChapterInsights`: the insights saved in the folder of `doc`, `None` when it
/// holds no file yet, or why the file could not be found or read. The folder is the one the
/// catalogue gave, when it holds this document's chapter, and else the one found under the
/// content folder. No store and no model is reached.
pub(in crate::backend::live) async fn load_chapter_insights<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    doc: DocId,
    folder: Option<PathBuf>,
    reply: &Reply,
) {
    let content_folder = cx.config().content_folder.clone();
    let searched = content_folder.clone();
    let read =
        tokio::task::spawn_blocking(move || saved_on_disk(doc, folder.as_deref(), &searched)).await;
    let result = match read {
        Ok(Ok(saved)) => Ok(saved.map(chapter_insights)),
        Ok(Err(fault)) => Err(fault.failure(doc, &content_folder)),
        Err(error) => Err(Failure::internal(format!(
            "the insights reader stopped: {error}"
        ))),
    };
    reply.send(Event::ChapterInsights { request, result });
}

#[derive(Debug)]
enum InsightsFault {
    NoFolder,
    Unreadable(ocr::ContentError),
}

impl InsightsFault {
    fn failure(self, doc: DocId, content_folder: &Path) -> Failure {
        match self {
            InsightsFault::NoFolder => Failure {
                detail: format!(
                    "no converted folder of document {} was found under {}",
                    doc.0,
                    content_folder.display()
                ),
                ..Failure::pages_not_found(content_folder)
            },
            InsightsFault::Unreadable(error) => {
                ChapterInsights::unreadable(doc, rag_core::error_chain(&error))
            }
        }
    }
}

fn saved_on_disk(
    doc: DocId,
    folder: Option<&Path>,
    content_folder: &Path,
) -> Result<Option<ocr::ChapterInsights>, InsightsFault> {
    let stored = rag_core::DocId::from(doc);
    let mut found = chapters_on_disk([(stored, folder)], content_folder);
    let entry = found.remove(&stored).ok_or(InsightsFault::NoFolder)?;
    ocr::ChapterInsights::read(&entry.folder).map_err(InsightsFault::Unreadable)
}

fn chapter_insights(saved: ocr::ChapterInsights) -> ChapterInsights {
    ChapterInsights {
        model: saved.model,
        insights: saved
            .insights
            .into_iter()
            .map(|insight| Insight {
                text: insight.text,
                pages: insight.pages,
            })
            .collect(),
        takeaways: saved.takeaways,
        cheat_sheet: saved.cheat_sheet,
    }
}
