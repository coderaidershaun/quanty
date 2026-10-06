//! The `rag-ingest` command: checks that the services are ready, ingests one converted chapter
//! folder, or deletes one document.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use graph::FalkorGraph;
use rag_core::{ClaudeCli, Config, DocId, GeminiEmbedder, ItemStore};
use rag_ingestion::{
    ConceptExtractor, EXTRACTION_MODEL, Models, Stores, delete_document, health, ingest_chapter,
};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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

    /// Remove one document, with its items, from Qdrant and from the graph
    DeleteDocument {
        /// The document id that an ingest prints, such as 5f3c2a1e-9b04-5d6e-8a17-2c4b7e90f1d3
        document_id: DocId,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    // `rag_core` logs what each `claude` question cost, and a person should see it as it happens.
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            Targets::new()
                .with_default(Level::WARN)
                .with_target("rag_core", Level::INFO),
        )
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
        (Some(Command::DeleteDocument { document_id }), _) => {
            delete(document_id).await.map(|()| ExitCode::SUCCESS)
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

async fn connect_stores(config: &Config) -> Result<Stores<FalkorGraph>> {
    Ok(Stores {
        items: ItemStore::connect(config).context("could not set up the item store")?,
        graph: FalkorGraph::connect(config)
            .await
            .context("could not connect to the graph")?,
    })
}

async fn ingest(folder: &Path) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    let models = Models {
        embedder: GeminiEmbedder::from_config(&config).context("could not set up the embedder")?,
        concepts: ConceptExtractor::new(
            ClaudeCli::new(EXTRACTION_MODEL),
            &config.concept_cache_folder,
        ),
    };
    let stores = connect_stores(&config).await?;
    let summary = ingest_chapter(folder, &models, &stores)
        .await
        .with_context(|| format!("could not ingest {}", folder.display()))?;
    println!("{summary}");
    Ok(())
}

async fn delete(document_id: DocId) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    let stores = connect_stores(&config).await?;
    let summary = delete_document(document_id, &stores)
        .await
        .with_context(|| format!("could not delete the document {document_id}"))?;
    println!("{summary}");
    Ok(())
}
