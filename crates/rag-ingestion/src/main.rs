//! The `rag-ingest` command: checks that the services are ready, ingests one converted chapter
//! folder or one picture that stands alone, converts and ingests one chapter PDF, or deletes one
//! document.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use graph::FalkorGraph;
use ocr::ChapterJob;
use rag_core::{ApiKey, ClaudeCli, ConceptStore, Config, DocId, GeminiEmbedder, ItemStore};
use rag_ingestion::{
    ChapterPdf, ConceptExtractor, EXTRACTION_MODEL, IngestSummary, LoneImage, Models, Stores,
    delete_document, health, ingest_chapter, ingest_image, ingest_pdf,
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
    #[arg(
        help = "A converted chapter folder, such as content/<book>/chapter-1, or a PNG or JPEG file that stands alone, such as a chart"
    )]
    path: Option<PathBuf>,

    #[arg(
        long,
        help = "What you know about the picture. It is added to the explanation of the picture"
    )]
    note: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Check that Qdrant and FalkorDB answer and that the claude CLI is signed in
    Health,

    /// Convert one chapter PDF and ingest it, in one run that needs no one
    Pdf {
        /// The title of the book that the chapter is from
        #[arg(long)]
        book: String,

        // This help text is an attribute and not a doc comment, because rustdoc takes `<number>`
        // for an HTML tag that is never closed.
        #[arg(help = "The chapter PDF, named chapter-<number>-<name>.pdf")]
        chapter_pdf: PathBuf,
    },

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
    match (cli.command, cli.path) {
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
        (Some(Command::Pdf { book, chapter_pdf }), _) => convert_and_ingest(&book, &chapter_pdf)
            .await
            .map(|()| ExitCode::SUCCESS),
        (Some(Command::DeleteDocument { document_id }), _) => {
            delete(document_id).await.map(|()| ExitCode::SUCCESS)
        }
        (None, Some(path)) => ingest(&path, cli.note.as_deref())
            .await
            .map(|()| ExitCode::SUCCESS),
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
        concepts: ConceptStore::connect(config).context("could not set up the concept store")?,
    })
}

fn set_up_models(config: &Config) -> Result<Models<GeminiEmbedder, ClaudeCli>> {
    Ok(Models {
        embedder: GeminiEmbedder::from_config(config).context("could not set up the embedder")?,
        concepts: ConceptExtractor::new(ClaudeCli::new(EXTRACTION_MODEL), config),
    })
}

/// What the person gave `rag-ingest` to ingest.
enum Source<'a> {
    Chapter(&'a Path),
    Picture(&'a Path),
}

impl<'a> Source<'a> {
    /// Looks at the path and the note before anything is started, so a mistake costs nothing.
    fn of(path: &'a Path, note: Option<&str>) -> Result<Source<'a>> {
        if path.is_dir() {
            if note.is_some() {
                bail!(
                    "--note is for a picture, and {} is a folder; a chapter takes no note",
                    path.display()
                );
            }
            Ok(Source::Chapter(path))
        } else if path.is_file() {
            Ok(Source::Picture(path))
        } else {
            bail!(
                "{} is not there; give a converted chapter folder, such as content/<book>/chapter-1, or a PNG or JPEG file",
                path.display()
            )
        }
    }
}

async fn ingest(path: &Path, note: Option<&str>) -> Result<()> {
    let source = Source::of(path, note)?;
    let config = Config::load().context("could not read the settings")?;
    // The models are set up before a picture is converted, so a missing key for the embedder
    // fails before the model that reads the picture is paid for.
    let models = set_up_models(&config)?;
    let stores = connect_stores(&config).await?;
    let summary: IngestSummary = match source {
        Source::Chapter(folder) => ingest_chapter(folder, &models, &stores)
            .await
            .with_context(|| format!("could not ingest {}", folder.display()))?,
        Source::Picture(file) => {
            let image = ocr::convert_image(file, &config.content_folder)
                .await
                .with_context(|| format!("could not convert {}", file.display()))?;
            ingest_image(
                &LoneImage {
                    image: &image,
                    note,
                },
                &models,
                &stores,
            )
            .await
            .with_context(|| format!("could not ingest {}", file.display()))?
        }
    };
    println!("{summary}");
    Ok(())
}

async fn convert_and_ingest(book: &str, chapter_pdf: &Path) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    // Both checks come before anything is set up, so a mistake costs nothing.
    let job = ChapterJob::new(book, chapter_pdf, &config.content_folder).with_context(|| {
        format!(
            "could not set up the conversion of {}",
            chapter_pdf.display()
        )
    })?;
    if !chapter_pdf.is_file() {
        bail!(
            "{} is not there; give the chapter pdf, named chapter-<number>-<name>.pdf",
            chapter_pdf.display()
        );
    }
    // The models are set up before any page is converted, so a missing key for the embedder
    // fails before the first page is paid for.
    let models = set_up_models(&config)?;
    let stores = connect_stores(&config).await?;
    // The key goes to the converter as a value, because the settings never enter the process
    // environment.
    let jev_api_key = config.jev_api_key.as_ref().map(ApiKey::expose);
    let outcome = ingest_pdf(
        ChapterPdf {
            job: &job,
            convert: async |job: &ChapterJob| {
                ocr::convert_chapter_with_jev_key(job, jev_api_key).await
            },
        },
        &models,
        &stores,
    )
    .await
    .with_context(|| format!("could not ingest {}", chapter_pdf.display()))?;
    println!("{outcome}");
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
