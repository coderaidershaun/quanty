//! The paid work of one ingest, the same steps as `rag-ingest pdf`: set up the models and the
//! stores, convert and ingest the document, and write its own tags.

use ocr::{ChapterJob, PageProgress};
use rag_core::MediaLabels;
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
            on_step: |step| {
                if let Some(progress) = line.after(step) {
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
    pages_done: u32,
    pages: u32,
    cost_usd: f64,
    pages_failed: u32,
}

impl ProgressLine {
    /// `None` for a failed page: it is counted, and shows with the next step.
    fn after(&mut self, step: IngestStep) -> Option<IngestProgress> {
        let (stage, done, total) = match step {
            IngestStep::Converting(PageProgress::Pages { total, done_before }) => {
                self.pages = total;
                self.pages_done = done_before;
                (IngestStage::PreparingPages, None, Some(total))
            }
            IngestStep::Converting(PageProgress::PageDone { cost_usd, .. }) => {
                self.pages_done += 1;
                self.cost_usd += cost_usd;
                (
                    IngestStage::Converting,
                    Some(self.pages_done),
                    Some(self.pages),
                )
            }
            IngestStep::Converting(PageProgress::PageFailed { .. }) => {
                self.pages_failed += 1;
                return None;
            }
            IngestStep::WritingGraph => (IngestStage::WritingGraph, None, None),
            IngestStep::Embedding { items } => (IngestStage::Embedding, None, Some(items as u32)),
            IngestStep::Storing => (IngestStage::Storing, None, None),
            IngestStep::ReadingConcepts { done, total } => (
                IngestStage::ReadingConcepts,
                Some(done as u32),
                Some(total as u32),
            ),
            IngestStep::LinkingConcepts { done, total } => (
                IngestStage::LinkingConcepts,
                Some(done as u32),
                Some(total as u32),
            ),
        };
        Some(IngestProgress {
            stage,
            done,
            total,
            pages_failed: self.pages_failed,
            cost_usd: self.cost_usd,
        })
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
        // A chapter that was found converted cost nothing in this run.
        cost_usd: (conversion.converted_now > 0).then_some(conversion.calls.cost_usd),
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
