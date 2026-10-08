//! The paid work of one ingest job, the same steps that `rag-ingest pdf` takes: save an uploaded
//! PDF, set up the models and the stores, convert and ingest the chapter, and write the labels.

use std::sync::Arc;

use graph::FalkorGraph;
use ocr::ChapterJob;
use rag_core::{ConceptStore, Config, ItemStore};
use rag_ingestion::{
    ChapterPdf, ConceptExtractor, EXTRACTION_MODEL, Models, PdfOutcome, Stores, ingest_pdf, relabel,
};
use tokio::sync::watch;

use super::PdfIngestError;
use super::jobs::{IngestReport, Stage};
use super::pdf::{CheckedPdf, Upload};
use crate::services::Services;

/// One ingest job, with everything it needs to run on a task of its own.
pub(super) struct Work<S> {
    pub(super) services: Arc<S>,
    pub(super) config: Config,
    pub(super) pdf: CheckedPdf,
    /// Where the stage is written. The end of the report is written by the task that waits for
    /// this one, not here.
    pub(super) report: watch::Sender<IngestReport>,
}

impl<S: Services> Work<S> {
    pub(super) async fn run(mut self) -> Result<PdfOutcome, PdfIngestError> {
        if let Some(upload) = self.pdf.upload.take() {
            // Writing megabytes to the disk can take a while, so it runs on a thread that may
            // block and the other calls are not held up.
            tokio::task::spawn_blocking(move || save(&upload))
                .await
                .map_err(PdfIngestError::Stopped)??;
        }
        let Work {
            services,
            config,
            pdf,
            report,
        } = &self;
        // The models are set up before the first page is converted, so a missing key for the
        // embedder fails before anything is paid for.
        let models = Models {
            embedder: services.embedder(config)?,
            concepts: ConceptExtractor::new(services.llm(EXTRACTION_MODEL), config),
        };
        let stores = Stores {
            items: ItemStore::connect(config).map_err(PdfIngestError::ItemStore)?,
            graph: FalkorGraph::connect(config).await?,
            concepts: ConceptStore::connect(config).map_err(PdfIngestError::ConceptStore)?,
        };
        let outcome = ingest_pdf(
            ChapterPdf {
                job: &pdf.chapter,
                convert: async |chapter: &ChapterJob| {
                    report.send_modify(|report| report.stage = Some(Stage::Converting));
                    let summary = services.convert(chapter, config).await?;
                    report.send_modify(|report| report.stage = Some(Stage::Ingesting));
                    Ok(summary)
                },
            },
            &models,
            &stores,
        )
        .await
        .map_err(|error| PdfIngestError::Pdf(Box::new(error)))?;
        // The labels come after the ingest, so a run that stops in the ingest labels nothing,
        // and the same PDF sent again finishes both.
        if !pdf.labels.is_empty() {
            relabel(outcome.doc_id(), &pdf.labels, &stores).await?;
        }
        Ok(outcome)
    }
}

fn save(upload: &Upload) -> Result<(), PdfIngestError> {
    let failed = |source| PdfIngestError::Upload {
        path: upload.save_to.clone(),
        source,
    };
    if let Some(folder) = upload.save_to.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    std::fs::write(&upload.save_to, &upload.bytes).map_err(failed)
}
