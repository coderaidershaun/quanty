//! What a run of `ingest_pdf` does, with stand-ins for the paid calls of `ocr`, a throwaway
//! collection of the local Qdrant and a throwaway graph of the local FalkorDB: one run stores the
//! whole chapter, a second run does nothing, and a run that stopped is finished by the next one.

use std::collections::BTreeSet;

use graph::GraphStore;
use graph::testing::stored_document;
use ocr::testing::Scenario;
use ocr::{ConvertError, read_chapter};
use rag_core::{DocId, ItemKind};
use rag_ingestion::{ConceptError, IngestError, PdfError, PdfOutcome, chapter_items};

use super::{
    StandInPdf, at_the_usage_limit, chain_of, finding_volatility, held_by, printed_page_of,
};
use crate::support::{StandInLlm, ThrowawayStores, assert_document_stored, points_in};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_chapter_pdf_is_converted_and_ingested_in_one_run_and_a_second_run_does_nothing() {
    let throwaway = ThrowawayStores::new("pdf-one-run");
    let config = throwaway.config();
    let stores = throwaway.connect().await;
    let pdf = StandInPdf::new(&throwaway, Scenario::SampleChapter);
    let model = StandInLlm::replying(|_, _| Ok(finding_volatility()));
    let models = throwaway.models(model.clone());

    let first = pdf.ingest(&models, &stores).await.unwrap();

    let PdfOutcome::Ingested(summary) = &first else {
        panic!("expected the pdf to be ingested, got {first}");
    };
    assert_eq!(summary.conversion.page_count, 7);
    assert_eq!(summary.conversion.converted_now, 7);
    let chapter_folder = pdf.job.chapter_folder();
    assert!(chapter_folder.starts_with(&config.content_folder));
    assert!(chapter_folder.join("chapter.json").is_file());
    let items = chapter_items(&read_chapter(&chapter_folder).unwrap());
    let n = items.len();
    let printed = first.to_string();
    assert!(
        printed.contains("pages: 7 (7 converted now, 0 already done)"),
        "{printed}"
    );
    let of_kind = |kind: ItemKind| {
        items
            .iter()
            .filter(|item| item.payload.kind == kind)
            .count()
    };
    assert!(
        printed.contains(&format!(
            "items: {} chunks, {} formulas, {} figures, {} tables ({n} in all)",
            of_kind(ItemKind::Chunk),
            of_kind(ItemKind::Formula),
            of_kind(ItemKind::Figure),
            of_kind(ItemKind::Table),
        )),
        "{printed}"
    );
    assert!(
        printed.contains(&format!(
            "concepts: 1 created, {} linked to an existing concept",
            n - 1
        )),
        "{printed}"
    );

    let points = points_in(config).await;
    assert_eq!(points.len(), n);
    let all_pages: BTreeSet<u64> = (1..=7).collect();
    let point_pages: BTreeSet<u64> = points
        .iter()
        .map(|(_, payload)| payload["page"].as_u64().unwrap())
        .collect();
    assert_eq!(point_pages, all_pages);
    for (id, payload) in &points {
        let page = payload["page"].as_u64().unwrap();
        assert_eq!(
            payload["printed_page"],
            printed_page_of(page).as_str(),
            "the point {id} of page {page}"
        );
    }
    let stored = assert_document_stored(&stores.graph, &items).await;
    let node_pages: BTreeSet<u64> = stored.items.iter().map(|item| item.page as u64).collect();
    assert_eq!(
        node_pages, all_pages,
        "the chain of NEXT edges crosses every page break"
    );
    for item in &stored.items {
        assert_eq!(
            item.printed_page.as_deref(),
            Some(printed_page_of(item.page as u64).as_str()),
            "the node {} of page {}",
            item.id,
            item.page
        );
    }

    let doc_id = summary.ingest.doc_id;
    let calls_before = (
        pdf.stubs.calls().len(),
        models.embedder.received().len(),
        model.calls(),
        pdf.conversions_started(),
    );
    let held_before = held_by(&throwaway, &stores, doc_id).await;

    let second = pdf.ingest(&models, &stores).await.unwrap();

    assert_eq!(
        second,
        PdfOutcome::AlreadyIngested {
            doc_id,
            items: n as u64
        }
    );
    let printed = second.to_string();
    assert!(printed.contains("already ingested"), "{printed}");
    assert!(printed.contains(&doc_id.to_string()), "{printed}");
    let calls_after = (
        pdf.stubs.calls().len(),
        models.embedder.received().len(),
        model.calls(),
        pdf.conversions_started(),
    );
    assert_eq!(
        calls_after, calls_before,
        "a second run converts, embeds and asks nothing"
    );
    assert_eq!(
        held_by(&throwaway, &stores, doc_id).await,
        held_before,
        "a second run writes nothing"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_run_that_stops_in_conversion_or_in_ingestion_is_finished_by_the_same_command() {
    let throwaway = ThrowawayStores::new("pdf-resume");
    let config = throwaway.config();
    let stores = throwaway.connect().await;
    let answering = StandInLlm::replying(|_, _| Ok(finding_volatility()));
    let models = throwaway.models(answering.clone());

    // A page that fails stops the run and is named. Nothing is embedded or asked, and the graph
    // does not hear of the document.
    let broken = StandInPdf::new(
        &throwaway,
        Scenario::AllTables {
            broken_page: Some(7),
        },
    );
    let document = DocId::from_source_sha256(&broken.job.source_sha256().unwrap());
    let error = broken.ingest(&models, &stores).await.unwrap_err();
    assert!(
        matches!(
            error,
            PdfError::Convert(ConvertError::PageFailed { position: 7, .. })
        ),
        "{error:?}"
    );
    let message = chain_of(&error);
    assert!(message.contains("page 7"), "{message}");
    assert!(models.embedder.received().is_empty());
    assert_eq!(answering.calls(), 0);
    assert!(
        stored_document(&stores.graph, document).await.is_none(),
        "the graph has no such document"
    );
    assert_eq!(
        stores.items.count().await.unwrap(),
        0,
        "the items collection was made before the first page was converted"
    );

    // The same command, with the page now working, goes on from the pages that were kept and
    // stops in the ingest, because the model is at its usage limit. The items are stored, and the
    // document is not marked as ingested.
    let working = StandInPdf::new(&throwaway, Scenario::AllTables { broken_page: None });
    let limited = throwaway.models(at_the_usage_limit());
    let error = working.ingest(&limited, &stores).await.unwrap_err();
    assert!(
        matches!(
            error,
            PdfError::Ingest(IngestError::Concepts(ConceptError::Stopped { .. }))
        ),
        "{error:?}"
    );
    let calls = working.stubs.calls();
    assert!(!calls.is_empty());
    assert!(
        calls.iter().all(|call| call.position() == 7),
        "pages 1 to 6 were kept: {calls:?}"
    );
    let n = chapter_items(&read_chapter(&working.job.chapter_folder()).unwrap()).len() as u64;
    assert_eq!(points_in(config).await.len() as u64, n);
    assert_eq!(stores.graph.ingested_items(document).await.unwrap(), None);

    // The same command again, with a model that answers, finishes the ingest. Nothing is
    // converted again.
    let calls_before = working.stubs.calls().len();
    let finished = working.ingest(&models, &stores).await.unwrap();
    let PdfOutcome::Ingested(summary) = &finished else {
        panic!("expected the pdf to be ingested, got {finished}");
    };
    assert_eq!(summary.conversion.converted_now, 0);
    assert_eq!(working.stubs.calls().len(), calls_before);
    assert!(summary.ingest.concepts.skipped_items.is_empty());
    assert_eq!(
        stores.graph.ingested_items(document).await.unwrap(),
        Some(n)
    );

    let again = working.ingest(&models, &stores).await.unwrap();
    assert_eq!(
        again,
        PdfOutcome::AlreadyIngested {
            doc_id: document,
            items: n
        }
    );

    // A delete that stopped after the points and before the nodes leaves the mark in the graph
    // and no point in the collection. The mark alone is not believed.
    assert_eq!(stores.items.delete_document(document).await.unwrap(), n);
    assert_eq!(
        stores.graph.ingested_items(document).await.unwrap(),
        Some(n)
    );
    let repaired = working.ingest(&models, &stores).await.unwrap();
    assert!(
        matches!(repaired, PdfOutcome::Ingested(_)),
        "expected the pdf to be ingested again, got {repaired}"
    );
    assert_eq!(stores.items.count_document(document).await.unwrap(), n);
    let last = working.ingest(&models, &stores).await.unwrap();
    assert!(matches!(last, PdfOutcome::AlreadyIngested { .. }), "{last}");
}
