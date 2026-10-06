//! The `rag-ingest` command: checks that the services are ready, or ingests one converted
//! chapter folder.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use rag_core::{Config, GeminiEmbedder, ItemStore};
use rag_ingestion::{health, ingest_chapter};
use tracing_subscriber::filter::LevelFilter;

#[derive(Parser)]
#[command(name = "rag-ingest", args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    // This help text is an attribute and not a doc comment, because rustdoc takes `<book>` for
    // an HTML tag that is never closed.
    #[arg(help = "A converted chapter folder, such as content/<book>/chapter-1")]
    chapter_folder: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Check that Qdrant and FalkorDB answer and that the claude CLI is signed in
    Health,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::WARN)
        .with_writer(std::io::stderr)
        .init();
    match run(Cli::parse()).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    match (cli.command, cli.chapter_folder) {
        (Some(Command::Health), _) => {
            let config = Config::load().context("could not read the settings")?;
            let report = health::check(&config).await;
            println!("{report}");
            Ok(if report.is_healthy() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        (None, Some(folder)) => ingest(&folder).await.map(|()| ExitCode::SUCCESS),
        (None, None) => {
            Cli::command()
                .print_help()
                .context("could not print the help text")?;
            Ok(ExitCode::from(2))
        }
    }
}

async fn ingest(folder: &Path) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    let embedder = GeminiEmbedder::from_config(&config).context("could not set up the embedder")?;
    let store = ItemStore::connect(&config).context("could not set up the item store")?;
    let summary = ingest_chapter(folder, &embedder, &store)
        .await
        .with_context(|| format!("could not ingest {}", folder.display()))?;
    println!("{summary}");
    Ok(())
}
