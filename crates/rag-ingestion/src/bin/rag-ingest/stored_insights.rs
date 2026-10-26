//! The `rag-ingest insights` command: writes the missing insights, takeaways and cheat sheets of
//! chapters that are stored already, or lists what it would write. It needs the graph and the
//! content folder only, so nothing is embedded.

use std::process::ExitCode;

use anyhow::{Context, Result};
use graph::FalkorGraph;
use ocr::content::INSIGHTS_FILE;
use rag_core::{AgentCli, Config, Llm, UsageTally, WorkKind, cost_text};
use rag_ingestion::{
    ChapterChoice, InsightsNeed, InsightsOutcome, InsightsWriter, StoredChapter, stored_chapters,
};

const ALREADY_SAVED: &str = "insights already saved";

/// Prints one line for each chosen chapter and, for the files that are missing, says that they
/// would be written. No model is asked.
pub(super) async fn list(choice: ChapterChoice<'_>) -> Result<ExitCode> {
    let (config, chapters) = chosen(choice).await?;
    let (mut to_write, mut saved, mut without_chapter, mut book_parts) = (0, 0, 0, 0);
    for chapter in &chapters {
        let what = match chapter.need() {
            InsightsNeed::BookPart => {
                book_parts += 1;
                InsightsOutcome::NoneForBookPart.line(chapter.doc_id)
            }
            InsightsNeed::Write(folder) => {
                to_write += 1;
                format!("would write {}", folder.join(INSIGHTS_FILE).display())
            }
            InsightsNeed::AlreadySaved => {
                saved += 1;
                ALREADY_SAVED.to_owned()
            }
            InsightsNeed::NoChapter => {
                without_chapter += 1;
                no_chapter_line(&config)
            }
        };
        println!("{}: {what}", label(chapter));
    }
    println!(
        "dry run: {to_write} to write, {saved} already saved, {without_chapter} without a converted chapter, {book_parts} parts of a book with none; no model was asked"
    );
    Ok(ExitCode::SUCCESS)
}

/// Writes the missing files one chapter after another. It succeeds when every chosen chapter that
/// gets insights has its file at the end.
pub(super) async fn write(choice: ChapterChoice<'_>) -> Result<ExitCode> {
    let (config, chapters) = chosen(choice).await?;
    let mut writer = None;
    let mut usage = UsageTally::default();
    let (mut written, mut saved, mut not_written, mut book_parts) = (0, 0, 0, 0);
    // SMELL: after an error that would fail every question, such as a usage limit, each chapter
    // that is left is still asked and fails at once. Stopping there is not safe yet, because every
    // Codex error, a slow answer included, counts as such an error.
    for chapter in &chapters {
        let what = match chapter.need() {
            InsightsNeed::Write(folder) => {
                let writer = match &mut writer {
                    Some(writer) => writer,
                    unready => unready.insert(ready_writer().await?),
                };
                let summary = writer.write_missing(folder).await;
                usage.add_all(&summary.usage);
                match summary.outcome {
                    InsightsOutcome::Written { .. } => written += 1,
                    InsightsOutcome::AlreadySaved { .. } => saved += 1,
                    InsightsOutcome::NotWritten { .. } => not_written += 1,
                    InsightsOutcome::NoneForBookPart => book_parts += 1,
                }
                summary.outcome.line(chapter.doc_id)
            }
            InsightsNeed::BookPart => {
                book_parts += 1;
                InsightsOutcome::NoneForBookPart.line(chapter.doc_id)
            }
            InsightsNeed::AlreadySaved => {
                saved += 1;
                ALREADY_SAVED.to_owned()
            }
            InsightsNeed::NoChapter => {
                not_written += 1;
                no_chapter_line(&config)
            }
        };
        println!("{}: {what}", label(chapter));
    }
    println!(
        "written: {written}, already saved: {saved}, not written: {not_written}, parts of a book with none: {book_parts}"
    );
    if !usage.is_empty() {
        println!("{usage}");
    }
    println!("cost of this run: {}", cost_text(usage.cost_usd()));
    Ok(if not_written == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

async fn chosen(choice: ChapterChoice<'_>) -> Result<(Config, Vec<StoredChapter>)> {
    let config = Config::load().context("could not read the settings")?;
    let graph = FalkorGraph::connect(&config)
        .await
        .context("could not connect to the graph")?;
    let chapters = stored_chapters(choice, &graph, &config.content_folder)
        .await
        .context("could not find the stored chapters")?;
    Ok((config, chapters))
}

/// The agent is checked once, which costs nothing, so one that is not signed in fails the command
/// before the first chapter is asked about. A run with no chapter to write never starts it.
async fn ready_writer() -> Result<InsightsWriter<AgentCli>> {
    let agent = AgentCli::for_work(WorkKind::Ingest);
    agent
        .check_ready()
        .await
        .context("the agent of the Ingest choice cannot be asked, so no insights were written")?;
    Ok(InsightsWriter::new(agent))
}

fn label(chapter: &StoredChapter) -> String {
    format!("{} ({})", chapter.title, chapter.doc_id)
}

fn no_chapter_line(config: &Config) -> String {
    format!(
        "no converted chapter under {}",
        config.content_folder.display()
    )
}
