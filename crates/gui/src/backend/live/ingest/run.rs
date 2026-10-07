//! The paid work of one ingest, the same steps as `rag-ingest pdf`: set up the models and the
//! stores, convert and ingest the chapter, and write the labels.

use ocr::ChapterJob;
use rag_ingestion::{ChapterPdf, LabelChange, PdfOutcome, PdfSummary, ingest_pdf, relabel};

use super::chapter_job;
use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{
    Event, Failure, IngestOutcome, IngestProgress, IngestReport, IngestRequest, IngestStage,
    ItemCounts, PageToCheck, RequestId,
};

/// Sends the progress of the run, then exactly one `Event::IngestFinished`, last.
///
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
    let (_, job) = chapter_job(cx, ingest)?;
    let labels = label_change(ingest);
    // The models come before the first page, so a missing key for the embedder fails before
    // anything is paid for.
    let models = cx.models()?;
    let stores = cx.stores().await?;
    let outcome = ingest_pdf(
        ChapterPdf {
            job: &job,
            convert: async |chapter: &ChapterJob| {
                send_progress(reply, request, IngestStage::Converting, 0.0);
                let summary = cx.convert_chapter(chapter).await?;
                let cost = summary.calls.cost_usd;
                send_progress(reply, request, IngestStage::WritingGraph, cost);
                Ok(summary)
            },
        },
        &models,
        &stores,
    )
    .await
    .map_err(|error| cx.failure(error))?;
    // The labels come after the ingest, whatever it found, so the same PDF started again
    // finishes a run that stopped before them.
    if !labels.is_empty() {
        relabel(outcome.doc_id(), &labels, &stores)
            .await
            .map_err(|error| cx.failure(error))?;
    }
    Ok(report(outcome))
}

fn send_progress(reply: &Reply, request: RequestId, stage: IngestStage, cost_usd: f64) {
    reply.send(Event::IngestProgress {
        request,
        progress: IngestProgress {
            stage,
            done: None,
            total: None,
            pages_failed: 0,
            cost_usd,
        },
    });
}

/// The author and the tags of the request. A blank author is none, and a blank tag is dropped.
fn label_change(ingest: &IngestRequest) -> LabelChange {
    LabelChange {
        author: ingest
            .author
            .as_deref()
            .map(str::trim)
            .filter(|author| !author.is_empty())
            .map(str::to_owned),
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
