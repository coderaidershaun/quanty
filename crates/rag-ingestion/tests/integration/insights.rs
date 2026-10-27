//! Runs `rag-ingest insights` as a person would, against throwaway stores and chapters that were
//! stored with no insights. The model is a stand-in `claude` that prints a made-up reply, so
//! nothing is billed.

use std::path::Path;

use ocr::ChapterInsights;
use rag_ingestion::{InsightsOutcome, InsightsWriter, ingest_chapter};

use crate::delete::copy_sample_chapter;
use crate::support::{
    self, RunRagIngest, StandInLlm, ThrowawayStores, points_in, stand_in_claude_runs,
};

fn has_insights(chapter: &Path) -> bool {
    chapter.join("insights.json").is_file()
}

pub(crate) fn lines_of(output: &std::process::Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored insights::"]
async fn rag_ingest_insights_writes_the_missing_files_of_stored_chapters() {
    let throwaway = ThrowawayStores::new("insights-command");
    let config = throwaway.config();
    let content = config.content_folder.clone();
    let stores = throwaway.connect().await;
    let book = copy_sample_chapter(&support::sample_chapter(), &content);
    let notes_1 = copy_sample_chapter(&support::intuition_chapter(), &content);
    let notes_2 = copy_sample_chapter(&support::in_depth_chapter(), &content);
    // The committed notes chapter carries its insights for the Library's Insights view, and this
    // run is about chapters that have none.
    std::fs::remove_file(notes_2.join(ocr::content::INSIGHTS_FILE))
        .expect("the committed notes chapter carries its insights");
    // These models write no insights, so the three chapters are stored without them.
    let models = throwaway.models(StandInLlm::finding_nothing());
    let mut book_id = None;
    for folder in [&book, &notes_1, &notes_2] {
        let summary = ingest_chapter(support::chapter_at(folder), &models, &stores)
            .await
            .unwrap();
        book_id.get_or_insert(summary.doc_id);
    }
    let book_id = book_id.unwrap();
    let points_before = points_in(config).await;
    assert!(
        ![&book, &notes_1, &notes_2]
            .into_iter()
            .any(|folder| has_insights(folder))
    );

    let dry_run = throwaway.rag_ingest_writing_insights(["insights", "--all", "--dry-run"]);

    assert!(
        dry_run.status.success(),
        "{}",
        String::from_utf8_lossy(&dry_run.stderr)
    );
    let lines = lines_of(&dry_run);
    assert_eq!(lines.len(), 4, "{lines:?}");
    for (line, folder) in lines.iter().zip([&book, &notes_1, &notes_2]) {
        let file = folder.join("insights.json");
        assert!(
            line.ends_with(&format!("would write {}", file.display())),
            "{line}"
        );
    }
    assert_eq!(
        lines[3],
        "dry run: 3 to write, 0 already saved, 0 without a converted chapter, 0 parts of a book with none; no model was asked"
    );
    assert_eq!(
        stand_in_claude_runs(&throwaway),
        0,
        "a dry run asks no model"
    );
    assert!(!has_insights(&book));

    let one =
        throwaway.rag_ingest_writing_insights(["insights", "--document", &book_id.to_string()]);

    assert!(
        one.status.success(),
        "{}",
        String::from_utf8_lossy(&one.stderr)
    );
    let lines = lines_of(&one);
    assert!(
        lines[0].contains(&format!("({book_id}): insights: written to ")),
        "{lines:?}"
    );
    assert!(
        lines.contains(
            &"written: 1, already saved: 0, not written: 0, parts of a book with none: 0"
                .to_owned()
        ),
        "{lines:?}"
    );
    assert!(has_insights(&book) && !has_insights(&notes_1) && !has_insights(&notes_2));
    let saved = ChapterInsights::read(&book).unwrap().unwrap();
    assert_eq!(
        saved.model, "sonnet",
        "the model of the Ingest choice wrote it"
    );
    assert_eq!(saved.takeaways.len(), 1);
    assert_eq!(
        stand_in_claude_runs(&throwaway),
        2,
        "one check of the sign-in and one question"
    );

    let media =
        throwaway.rag_ingest_writing_insights(["insights", "--media", "QUANTY SAMPLE NOTES"]);

    assert!(
        media.status.success(),
        "{}",
        String::from_utf8_lossy(&media.stderr)
    );
    let lines = lines_of(&media);
    assert!(
        lines.contains(
            &"written: 2, already saved: 0, not written: 0, parts of a book with none: 0"
                .to_owned()
        ),
        "{lines:?}"
    );
    assert!(has_insights(&notes_1) && has_insights(&notes_2));
    assert_eq!(
        stand_in_claude_runs(&throwaway),
        5,
        "a check and two questions more"
    );

    let all = throwaway.rag_ingest_writing_insights(["insights", "--all"]);

    assert!(
        all.status.success(),
        "{}",
        String::from_utf8_lossy(&all.stderr)
    );
    let lines = lines_of(&all);
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.ends_with("insights already saved"))
            .count(),
        3,
        "{lines:?}"
    );
    assert!(
        lines.contains(
            &"written: 0, already saved: 3, not written: 0, parts of a book with none: 0"
                .to_owned()
        ),
        "{lines:?}"
    );
    assert_eq!(
        stand_in_claude_runs(&throwaway),
        5,
        "a run with nothing to write starts no model"
    );
    assert_eq!(
        points_in(config).await,
        points_before,
        "nothing was embedded or stored again"
    );
}

/// What the model wrote as the cheat sheet, and what is saved of it.
const CHEAT_SHEETS: [(&str, &str); 3] = [
    (
        r#"### Greeks\n- Delta: $\\frac{\\partial C}{\\partial S}$\n\n| a | b |\n|---|---|\n| 1 | 2 |""#,
        "### Greeks\n- Delta: $\\frac{\\partial C}{\\partial S}$\n\n| a | b |\n|---|---|\n| 1 | 2 |",
    ),
    (
        "### Rules\n- Vega is not \"volatility\".\n- $\\nu \\neq \\sigma$.\"",
        "### Rules\n- Vega is not \"volatility\".\n- $\\nu \\neq \\sigma$.",
    ),
    (
        "### Rules\n- $\\nu \\neq \\sigma$ and $a \\\\ b$.",
        "### Rules\n- $\\nu \\neq \\sigma$ and $a \\\\ b$.",
    ),
];

#[tokio::test]
async fn a_cheat_sheet_escaped_once_too_often_is_saved_as_markdown() {
    let content = tempfile::tempdir().unwrap();
    for (written, saved) in CHEAT_SHEETS {
        let chapter = copy_sample_chapter(&support::sample_chapter(), content.path());
        let mut reply = rag_ingestion::testing::stand_in_insights_reply();
        reply["cheat_sheet"] = written.into();
        let writer = InsightsWriter::new(StandInLlm::replying(move |_, _| Ok(reply.clone())));

        let summary = writer.write_missing(&chapter).await;

        assert!(
            matches!(summary.outcome, InsightsOutcome::Written { .. }),
            "{summary:?}"
        );
        let insights = ChapterInsights::read(&chapter).unwrap().unwrap();
        assert_eq!(insights.cheat_sheet, saved);
        std::fs::remove_dir_all(&chapter).unwrap();
    }
}
