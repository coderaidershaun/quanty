//! What a run of `ingest_pdf` does about the insights, the takeaways and the cheat sheet of the
//! chapter: it writes them once, asks nothing again, never lets a failed answer undo the stored
//! items, and asks nothing for a part of a book that is not a chapter.

use std::collections::BTreeSet;

use ocr::testing::{Scenario, sample_pdf};
use ocr::{ChapterInsights, ChapterJob, MediaDocument, read_chapter};
use rag_core::{Category, ItemKind, MediaLabels};
use rag_ingestion::testing::{AskedQuestion, stand_in_insights_reply};
use rag_ingestion::{
    IngestStep, InsightsOutcome, InsightsSummary, PdfOutcome, chapter_items,
    items_of_ingested_document,
};
use serde_json::json;

use super::{StandInPdf, held_by};
use crate::insights::lines_of;
use crate::support::{RunRagIngest, StandInLlm, ThrowawayStores, stand_in_claude_runs};

/// The questions about a whole chapter, told from the questions about one item by their schema.
fn insights_questions(model: &StandInLlm) -> Vec<AskedQuestion> {
    model
        .questions()
        .into_iter()
        .filter(|question| question.schema.contains("\"cheat_sheet\""))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_chapter_pdf_run_writes_its_insights_and_a_second_run_asks_nothing() {
    let throwaway = ThrowawayStores::new("pdf-insights");
    let stores = throwaway.connect().await;
    let pdf = StandInPdf::new(&throwaway, Scenario::SampleChapter);
    let model = StandInLlm::finding_nothing();
    let models = throwaway.models_writing_insights(model.clone());

    let first = pdf.ingest(&models, &stores).await.unwrap();

    let PdfOutcome::Ingested(summary) = &first else {
        panic!("expected the pdf to be ingested, got {first}");
    };
    let folder = pdf.job.chapter_folder();
    let file = std::fs::canonicalize(&folder)
        .unwrap()
        .join("insights.json");
    let Some(InsightsSummary {
        outcome: InsightsOutcome::Written { file: written },
        ..
    }) = &summary.ingest.insights
    else {
        panic!("expected the insights to be written: {:?}", summary.ingest);
    };
    assert_eq!(written, &file);
    let reply = stand_in_insights_reply();
    let saved = ChapterInsights::read(&folder).unwrap().unwrap();
    assert_eq!(saved.model, "stand-in");
    assert_eq!(saved.prompt_version, "1");
    assert_eq!(saved.insights.len(), 1);
    assert_eq!(saved.insights[0].text, reply["insights"][0]["text"]);
    assert_eq!(saved.insights[0].pages, [1, 2]);
    assert_eq!(saved.takeaways, [reply["takeaways"][0].as_str().unwrap()]);
    assert_eq!(saved.cheat_sheet, reply["cheat_sheet"]);
    let printed = first.to_string();
    assert!(
        printed.contains(&format!("insights: written to {}", file.display())),
        "{printed}"
    );

    let steps = pdf.steps();
    let storing = steps.iter().position(|step| *step == IngestStep::Storing);
    let writing = steps
        .iter()
        .position(|step| *step == IngestStep::WritingInsights);
    assert_eq!(writing, storing.map(|storing| storing + 1), "{steps:?}");

    let questions = insights_questions(&model);
    assert_eq!(questions.len(), 1, "one question for the whole chapter");
    let items = chapter_items(&read_chapter(&folder).unwrap());
    assert!(
        questions[0]
            .input
            .starts_with(&format!("Document: {}", items[0].payload.doc_title)),
        "{}",
        questions[0].input
    );
    let pages: BTreeSet<u32> = items.iter().map(|item| item.payload.page).collect();
    for page in &pages {
        let marker = format!("[page {page}");
        assert!(questions[0].input.contains(&marker), "{marker}");
    }
    for item in items
        .iter()
        .filter(|item| item.payload.kind == ItemKind::Chunk)
    {
        assert!(
            questions[0].input.contains(&item.payload.text),
            "the text of the item {} is missing from the question",
            item.id
        );
    }

    let calls_before = model.calls();
    let second = pdf.ingest(&models, &stores).await.unwrap();

    let PdfOutcome::AlreadyIngested {
        insights: Some(second_insights),
        ..
    } = &second
    else {
        panic!("expected the pdf to be already ingested, got {second}");
    };
    assert_eq!(
        second_insights.outcome,
        InsightsOutcome::AlreadySaved {
            file: folder.join("insights.json")
        }
    );
    assert_eq!(
        model.calls(),
        calls_before,
        "a second run asks no question, about the insights or anything else"
    );
    assert_eq!(insights_questions(&model).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_failed_insights_answer_keeps_the_stored_chapter_and_a_later_run_writes_only_the_file() {
    let throwaway = ThrowawayStores::new("pdf-insights-failed");
    let stores = throwaway.connect().await;
    let pdf = StandInPdf::new(&throwaway, Scenario::SampleChapter);
    // Every question gets the answer to a question about concepts, so the insights are refused.
    let answering_wrongly =
        StandInLlm::replying(|_, _| Ok(json!({ "concepts": [], "relations": [] })));
    let wrong_models = throwaway.models_writing_insights(answering_wrongly.clone());

    let first = pdf.ingest(&wrong_models, &stores).await.unwrap();

    let PdfOutcome::Ingested(summary) = &first else {
        panic!("expected the pdf to be ingested, got {first}");
    };
    let Some(InsightsSummary {
        outcome: InsightsOutcome::NotWritten { reason },
        ..
    }) = &summary.ingest.insights
    else {
        panic!(
            "expected the insights not to be written: {:?}",
            summary.ingest
        );
    };
    assert!(!reason.is_empty());
    let document = summary.ingest.doc_id;
    let printed = first.to_string();
    assert!(printed.contains("insights: not written: "), "{printed}");
    assert!(
        printed.contains(&format!("rag-ingest insights --document {document}")),
        "{printed}"
    );
    assert_eq!(
        insights_questions(&answering_wrongly).len(),
        2,
        "a refused answer is tried once more"
    );
    let folder = pdf.job.chapter_folder();
    assert_eq!(ChapterInsights::read(&folder).unwrap(), None);
    let n = chapter_items(&read_chapter(&folder).unwrap()).len() as u64;
    assert_eq!(
        items_of_ingested_document(document, &stores).await.unwrap(),
        Some(n),
        "the stored chapter is whole although its insights are not written"
    );

    let held_before = held_by(&throwaway, &stores, document).await;
    let writing = StandInLlm::finding_nothing();
    let models = throwaway.models_writing_insights(writing.clone());

    let second = pdf.ingest(&models, &stores).await.unwrap();

    assert!(
        matches!(
            &second,
            PdfOutcome::AlreadyIngested {
                insights: Some(InsightsSummary {
                    outcome: InsightsOutcome::Written { .. },
                    ..
                }),
                ..
            }
        ),
        "{second}"
    );
    assert_eq!(pdf.conversions_started(), 1, "nothing was converted again");
    assert!(ChapterInsights::read(&folder).unwrap().is_some());
    assert_eq!(
        writing.questions().len(),
        1,
        "the only question is the one about the insights"
    );
    assert_eq!(insights_questions(&writing).len(), 1);
    assert_eq!(
        held_by(&throwaway, &stores, document).await,
        held_before,
        "the points, the graph and the concepts are as they were"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn a_part_of_a_book_asks_nothing_and_gets_no_insights() {
    let throwaway = ThrowawayStores::new("pdf-insights-book-part");
    let stores = throwaway.connect().await;
    let index = MediaDocument::book_part("Option Volatility and Pricing", "Index");
    let job = ChapterJob::new(index, &sample_pdf(), &throwaway.config().content_folder).unwrap();
    let book = MediaLabels {
        category: Category::Book,
        ..MediaLabels::default()
    };
    let pdf = StandInPdf::of(job, book, Scenario::SampleChapter);
    let model = StandInLlm::finding_nothing();
    let models = throwaway.models_writing_insights(model.clone());

    let first = pdf.ingest(&models, &stores).await.unwrap();

    let PdfOutcome::Ingested(summary) = &first else {
        panic!("expected the pdf to be ingested, got {first}");
    };
    let Some(InsightsSummary {
        outcome: InsightsOutcome::NoneForBookPart,
        ..
    }) = &summary.ingest.insights
    else {
        panic!(
            "expected no insights for a part of a book: {:?}",
            summary.ingest
        );
    };
    let says_none = "insights: none, because a part of a book that is not a chapter gets none";
    let printed = first.to_string();
    assert!(printed.contains(says_none), "{printed}");
    assert!(insights_questions(&model).is_empty());
    let folder = pdf.job.chapter_folder();
    assert!(!folder.join("insights.json").exists());

    let calls_before = model.calls();
    let second = pdf.ingest(&models, &stores).await.unwrap();

    let PdfOutcome::AlreadyIngested {
        insights:
            Some(InsightsSummary {
                outcome: InsightsOutcome::NoneForBookPart,
                ..
            }),
        ..
    } = &second
    else {
        panic!("expected the pdf to be already ingested with no insights, got {second}");
    };
    assert!(second.to_string().contains(says_none), "{second}");
    assert_eq!(model.calls(), calls_before, "a second run asks nothing");
    assert!(insights_questions(&model).is_empty());
    assert!(
        !pdf.steps().contains(&IngestStep::WritingInsights),
        "neither run told the step"
    );

    let title = &summary.ingest.doc_title;
    let line = format!("{title} ({}): {says_none}", summary.ingest.doc_id);
    for arguments in [
        vec!["insights", "--media", "Option Volatility and Pricing"],
        vec!["insights", "--all", "--dry-run"],
    ] {
        let output = throwaway.rag_ingest_writing_insights(&arguments);
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let lines = lines_of(&output);
        assert!(lines.contains(&line), "{arguments:?}: {lines:?}");
    }
    assert_eq!(
        stand_in_claude_runs(&throwaway),
        0,
        "the command asked no model for a part of a book"
    );
    assert!(!folder.join("insights.json").exists());
}
