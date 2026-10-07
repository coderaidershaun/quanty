//! What a run of `ingest_pdf` does, with stand-ins for the paid calls of `ocr`, a throwaway
//! collection of the local Qdrant and a throwaway graph of the local FalkorDB: one run stores the
//! whole chapter, a second run does nothing, and a run that stopped or skipped an item is finished
//! by the next one.

use std::collections::BTreeSet;
use std::ffi::OsStr;

use graph::GraphStore;
use graph::testing::{size, stored_document};
use ocr::convert::convert_chapter_with;
use ocr::testing::{Scenario, sample_pdf};
use ocr::{ConvertError, read_chapter};
use rag_core::{DocId, ItemKind, LlmError};
use rag_ingestion::{
    ConceptError, IngestError, Item, Models, PdfError, PdfOutcome, chapter_items, ingest_chapter,
};

use super::{
    StandInPdf, at_the_usage_limit, chain_of, finding_volatility, held_by, printed_page_of,
};
use crate::support::{
    RunRagIngest, SAMPLE_BOOK, StandInLlm, ThrowawayStores, assert_document_stored,
    assert_labelled, points_in,
};

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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_document_with_a_skipped_item_or_a_later_stopped_ingest_is_not_taken_as_ingested() {
    let throwaway = ThrowawayStores::new("pdf-not-whole");
    let stores = throwaway.connect().await;
    let pdf = StandInPdf::new(&throwaway, Scenario::SampleChapter);

    // The first item of the sample chapter is the only one that is asked about text that no other
    // item has, so the model can fail for it alone.
    convert_chapter_with(&pdf.job, &pdf.stubs).await.unwrap();
    let items = chapter_items(&read_chapter(&pdf.job.chapter_folder()).unwrap());
    let n = items.len() as u64;
    let asked_about = |item: &Item| format!("{}\n\n{}", item.input.title, item.input.text);
    let odd_one_out = asked_about(&items[0]);
    assert_eq!(
        items
            .iter()
            .filter(|item| asked_about(item) == odd_one_out)
            .count(),
        1
    );
    let document = items[0].payload.doc_id;
    let model = StandInLlm::replying(move |input, earlier_calls| {
        if input == odd_one_out && earlier_calls < 2 {
            return Err(LlmError::Failed {
                exit_code: Some(1),
                reason: "Output blocked by content filtering policy".to_owned(),
            });
        }
        Ok(finding_volatility())
    });
    let models = throwaway.models(model.clone());

    // The item fails twice and is skipped: the run ends well, and the document is not marked.
    let first = pdf.ingest(&models, &stores).await.unwrap();
    let PdfOutcome::Ingested(summary) = &first else {
        panic!("expected the pdf to be ingested, got {first}");
    };
    assert_eq!(summary.ingest.concepts.skipped_items.len(), 1);
    assert_eq!(stores.graph.ingested_items(document).await.unwrap(), None);

    // The same command asks about that item again, and only that one. This time it is answered.
    let calls_before = model.calls();
    let second = pdf.ingest(&models, &stores).await.unwrap();
    let PdfOutcome::Ingested(summary) = &second else {
        panic!("expected the pdf to be ingested again, got {second}");
    };
    assert!(summary.ingest.concepts.skipped_items.is_empty());
    assert_eq!(model.calls(), calls_before + 1);
    assert_eq!(
        stores.graph.ingested_items(document).await.unwrap(),
        Some(n)
    );
    let third = pdf.ingest(&models, &stores).await.unwrap();
    assert!(
        matches!(third, PdfOutcome::AlreadyIngested { .. }),
        "{third}"
    );

    // A later ingest by hand that stops takes the mark away, so the next run does the work.
    let limited = throwaway.models(at_the_usage_limit());
    let limited = Models {
        concepts: limited.concepts.with_prompt_version("another version"),
        ..limited
    };
    let error = ingest_chapter(&pdf.job.chapter_folder(), &limited, &stores)
        .await
        .unwrap_err();
    assert!(
        matches!(error, IngestError::Concepts(ConceptError::Stopped { .. })),
        "{error:?}"
    );
    assert_eq!(stores.graph.ingested_items(document).await.unwrap(), None);
    let fourth = pdf.ingest(&models, &stores).await.unwrap();
    assert!(matches!(fourth, PdfOutcome::Ingested(_)), "{fourth}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_stored_document_is_labelled_in_place_by_pdf_and_by_tag_and_nothing_is_called() {
    let throwaway = ThrowawayStores::new("pdf-labels");
    let config = throwaway.config();
    let stores = throwaway.connect().await;
    let pdf = StandInPdf::new(&throwaway, Scenario::SampleChapter);
    let models = throwaway.models(StandInLlm::replying(|_, _| Ok(finding_volatility())));

    let first = pdf.ingest(&models, &stores).await.unwrap();
    let document = first.doc_id().to_string();
    let points = points_in(config).await;
    for (_, payload) in &points {
        assert_eq!(payload["book"], SAMPLE_BOOK, "{payload}");
    }
    let point_ids: BTreeSet<String> = points.into_iter().map(|(id, _)| id).collect();
    let graph_size = size(&stores.graph).await;

    // The built command finds the document stored, so it only writes the labels it was given.
    let sample = sample_pdf();
    let output = throwaway.rag_ingest_pdf([
        OsStr::new("--book"),
        OsStr::new(SAMPLE_BOOK),
        OsStr::new("--author"),
        OsStr::new("Sheldon Natenberg"),
        OsStr::new("--tag"),
        OsStr::new("Options"),
        sample.as_os_str(),
    ]);
    let (stdout, stderr) = (text_of(&output.stdout), text_of(&output.stderr));
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stdout.contains("already ingested"), "{stdout}");
    assert!(stdout.contains("author: Sheldon Natenberg"), "{stdout}");
    assert!(stdout.contains("tags: options"), "{stdout}");
    let labelled = assert_labelled(config, &stores.graph, "Sheldon Natenberg", &["options"]).await;
    assert_eq!(labelled.into_keys().collect::<BTreeSet<_>>(), point_ids);
    assert_eq!(size(&stores.graph).await, graph_size);

    let output = throwaway.rag_ingest([
        "tag",
        document.as_str(),
        "--author",
        "Another Author",
        "--add",
        "Greeks",
        "--remove",
        "options",
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text_of(&output.stderr));
    let labelled = assert_labelled(config, &stores.graph, "Another Author", &["greeks"]).await;
    assert_eq!(labelled.into_keys().collect::<BTreeSet<_>>(), point_ids);
    assert_eq!(size(&stores.graph).await, graph_size);

    let unknown = DocId::from_source_sha256("a document that no store holds").to_string();
    let output = throwaway.rag_ingest(["tag", unknown.as_str(), "--add", "options"]);
    let stderr = text_of(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains(&unknown), "{stderr}");
    assert_labelled(config, &stores.graph, "Another Author", &["greeks"]).await;

    // The stand-in `claude` adds a line to `calls` each time it runs. The `pdf` command had a
    // Gemini key that is not a real one and no converter key, so it ended well only because it
    // embedded and converted nothing.
    assert!(
        !throwaway
            .temporary_folder()
            .join("stand-in-claude/calls")
            .exists(),
        "no model was asked"
    );
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
