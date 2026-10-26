//! Explicit metadata backfill reads graph records and saved chapters without opening search stores.

use std::process::ExitCode;

use anyhow::{Context, Result};
use graph::FalkorGraph;
use rag_core::{AgentCli, Config, WorkKind};
use rag_ingestion::{
    ArtifactState, ArtifactWriter, ChapterChoice, assess_artifacts, stored_chapters,
};

pub(super) async fn run(choice: ChapterChoice<'_>, dry_run: bool) -> Result<ExitCode> {
    let config = Config::load().context("could not read settings")?;
    let graph = FalkorGraph::connect(&config)
        .await
        .context("could not connect to graph")?;
    let chapters = stored_chapters(choice, &graph, &config.content_folder)
        .await
        .context("could not find stored chapters")?;
    let writer = ArtifactWriter::new(AgentCli::for_work(WorkKind::Ingest));
    let mut unresolved = false;
    let mut usage = rag_core::UsageTally::default();
    for chapter in chapters {
        let Some(folder) = chapter.folder else {
            println!(
                "{} ({}): no-folder; restore converted source before backfill",
                chapter.title, chapter.doc_id
            );
            unresolved = true;
            continue;
        };
        let assessment = assess_artifacts(&folder);
        println!(
            "{} ({}): {}, {}/{}",
            chapter.title,
            chapter.doc_id,
            assessment.state.as_str(),
            assessment.done,
            assessment.total
        );
        if let Some(reason) = assessment.reason {
            println!("  {reason}");
        }
        if matches!(
            assessment.state,
            ArtifactState::TooLarge | ArtifactState::NoFolder
        ) {
            unresolved = true;
            continue;
        }
        if dry_run || assessment.state == ArtifactState::Ready {
            continue;
        }
        let mut document_usage = rag_core::UsageTally::default();
        let outcome = writer
            .write_missing_with_progress(&folder, &mut |step, spent| {
                document_usage = spent.clone();
                if let rag_ingestion::IngestStep::WritingArtifacts { done, total } = step {
                    eprintln!("artifacts: {done}/{total}\n{spent}");
                }
            })
            .await;
        usage.add_all(&document_usage);
        match outcome {
            Ok(summary) => println!(
                "artifacts: {} complete, {} written\n{}",
                summary.total, summary.written, summary.usage
            ),
            Err(error) => {
                eprintln!("{}: {}", chapter.doc_id, rag_core::error_chain(&error));
                unresolved = true;
                if error.stops_the_run() {
                    break;
                }
            }
        }
    }
    if dry_run {
        println!("dry run: no model was asked");
    } else {
        println!(
            "{usage}\ncost of this run: {}",
            rag_core::cost_text(usage.cost_usd())
        );
    }
    Ok(if unresolved {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
