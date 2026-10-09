//! The paid work of one ingest, the same steps as `rag-ingest pdf`: set up the models and the
//! stores, convert and ingest the document, and write its own tags.

use ocr::{ChapterJob, PageProgress};
use rag_core::{MediaLabels, UsageTally};
use rag_ingestion::{
    ChapterPdf, IngestStep, PdfOutcome, PdfSummary, TagChange, ingest_pdf, relabel_document_tags,
};

use super::chapter_job;
use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{
    Event, Failure, IngestOutcome, IngestProgress, IngestReport, IngestRequest, IngestStage,
    ItemCounts, PageToCheck, RequestId,
};

/// A cancel is not listened to: the work has no safe place to stop inside, so a run goes on to
/// its end.
pub async fn run<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    ingest: &IngestRequest,
    reply: &Reply,
) {
    let result = finish(cx, request, ingest, reply).await;
    reply.send(Event::IngestFinished { request, result });
}

async fn finish<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    ingest: &IngestRequest,
    reply: &Reply,
) -> Result<IngestOutcome, Failure> {
    let job = chapter_job(cx, ingest)?;
    let own_tags = tag_change(ingest);
    // The media is saved before its first document, so these are used only when it is missing.
    let new_media = MediaLabels {
        category: ingest.category.into(),
        ..MediaLabels::default()
    };
    // The models come before the first page, so a missing key for the embedder fails before
    // anything is paid for.
    let models = cx.models()?;
    let stores = cx.stores().await?;
    let mut line = ProgressLine::default();
    let outcome = ingest_pdf(
        ChapterPdf {
            job: &job,
            new_media: &new_media,
            convert:
                async |chapter: &ChapterJob, on_page: &mut (dyn FnMut(PageProgress) + Send + '_)| {
                    cx.convert_chapter(chapter, on_page).await
                },
            on_step: |step, spent: &UsageTally| {
                if let Some(progress) = line.after(step, spent) {
                    reply.send(Event::IngestProgress { request, progress });
                }
            },
        },
        &models,
        &stores,
    )
    .await
    .map_err(|error| cx.failure(error))?;
    // The own tags come after the ingest, whatever it found, so the same PDF started again
    // finishes a run that stopped before them.
    if !own_tags.is_empty() {
        relabel_document_tags(outcome.doc_id(), &own_tags, &stores)
            .await
            .map_err(|error| cx.failure(error))?;
    }
    Ok(report(outcome))
}

/// Turns the steps of one run into the progress the app shows, with the counts that run on from
/// one step to the next.
#[derive(Default)]
struct ProgressLine {
    pages_saved: u32,
    /// Pages saved before this run or cut out now.
    pages_ready: u32,
    pages_saved_before: u32,
    pages: u32,
}

impl ProgressLine {
    /// `None` for a failed page: the run ends with an error that names the page, so there is
    /// nothing to show for it here. `spent` is what the run used up to and with this step.
    fn after(&mut self, step: IngestStep, spent: &UsageTally) -> Option<IngestProgress> {
        let stage = match step {
            IngestStep::CheckingStored => IngestStage::CheckingStored,
            IngestStep::OpeningPdf => IngestStage::OpeningPdf,
            IngestStep::Converting(PageProgress::Pages { total, done_before }) => {
                self.pages = total;
                self.pages_saved = done_before;
                self.pages_ready = done_before;
                self.pages_saved_before = done_before;
                self.preparing_or_converting()
            }
            IngestStep::Converting(PageProgress::PageCut { .. }) => {
                self.pages_ready += 1;
                self.preparing_or_converting()
            }
            IngestStep::Converting(PageProgress::PageDone { .. }) => {
                self.pages_saved += 1;
                self.converting()
            }
            IngestStep::Converting(PageProgress::PageFailed { .. }) => return None,
            IngestStep::WritingGraph => IngestStage::WritingGraph,
            IngestStep::Embedding { items } => IngestStage::Embedding {
                items: items as u32,
            },
            IngestStep::Storing => IngestStage::Storing,
            IngestStep::ReadingConcepts { done, total } => IngestStage::ReadingConcepts {
                done: done as u32,
                items: total as u32,
            },
            IngestStep::LinkingConcepts { done, total } => IngestStage::LinkingConcepts {
                done: done as u32,
                items: total as u32,
            },
        };
        Some(IngestProgress {
            stage,
            spent: spent.into(),
        })
    }

    /// The stage after the pages are counted or one is cut out. The models start the moment the
    /// last page is cut, so the stage turns to converting then, not when the first page is saved.
    fn preparing_or_converting(&self) -> IngestStage {
        if self.pages_ready < self.pages {
            IngestStage::PreparingPages {
                ready: self.pages_ready,
                saved_before: self.pages_saved_before,
                pages: self.pages,
            }
        } else {
            self.converting()
        }
    }

    fn converting(&self) -> IngestStage {
        IngestStage::Converting {
            saved: self.pages_saved,
            pages: self.pages,
        }
    }
}

/// A blank tag is dropped.
fn tag_change(ingest: &IngestRequest) -> TagChange {
    TagChange {
        add: ingest
            .tags
            .iter()
            .filter_map(|tag| tag.parse().ok())
            .collect(),
        remove: Vec::new(),
    }
}

fn report(outcome: PdfOutcome) -> IngestOutcome {
    match outcome {
        PdfOutcome::AlreadyIngested { doc_id, items } => IngestOutcome::AlreadyIngested {
            doc: doc_id.into(),
            items,
            pages_to_check: None,
        },
        PdfOutcome::Ingested(summary) => IngestOutcome::Ingested(ingested(*summary)),
    }
}

fn ingested(summary: PdfSummary) -> IngestReport {
    let usage = (&summary.usage()).into();
    let PdfSummary { conversion, ingest } = summary;
    let counts = ingest.items_by_kind;
    IngestReport {
        doc: ingest.doc_id.into(),
        title: ingest.doc_title,
        pages: conversion.page_count,
        items: ItemCounts {
            chunks: counts.chunks as u64,
            formulas: counts.formulas as u64,
            figures: counts.figures as u64,
            tables: counts.tables as u64,
        },
        concepts_created: ingest.concepts.concepts_created,
        concepts_linked: ingest.concepts.concepts_linked,
        skipped_items: ingest.concepts.skipped_items.len(),
        usage,
        pages_to_check: conversion
            .pages_to_check
            .iter()
            .map(|page| PageToCheck {
                page: page.position,
                reasons: page
                    .reasons
                    .iter()
                    .map(|reason| (*reason).to_owned())
                    .collect(),
            })
            .collect(),
    }
}
