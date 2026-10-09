//! The `rag-ingest` command: checks that the services are ready, ingests a converted chapter or a
//! picture, converts and ingests a PDF, relabels a stored media or document, or deletes a
//! document.

mod cli;
mod progress;

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{CommandFactory, Parser};
use graph::FalkorGraph;
use ocr::{ChapterJob, PageProgress};
use rag_core::{
    ApiKey, Category, ClaudeCli, ConceptStore, Config, DocId, GeminiEmbedder, ItemStore,
    MediaLabels,
};
use rag_ingestion::{
    ChapterFolder, ChapterPdf, ConceptExtractor, EXTRACTION_MODEL, IngestSummary, LoneImage,
    MediaChange, Models, NamePdfError, Stores, TagChange, UnnamedPdf, delete_document, health,
    ingest_chapter, ingest_image, ingest_pdf, relabel_document_tags, relabel_media,
};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::cli::{Cli, Command, GivenPath, GivenPdf};
use crate::progress::print_step;

#[tokio::main]
async fn main() -> ExitCode {
    // `rag_core` logs what each `claude` question used, and a person should see it as it happens.
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
    match (cli.command, &cli.given.path) {
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
        (Some(Command::Pdf(given)), _) => {
            convert_and_ingest(&given).await.map(|()| ExitCode::SUCCESS)
        }
        (
            Some(Command::Tag {
                document_id,
                add,
                remove,
            }),
            _,
        ) => tag(document_id, &TagChange { add, remove })
            .await
            .map(|()| ExitCode::SUCCESS),
        (Some(Command::Media { title, labels }), _) => {
            relabel_stored_media(&title, &labels.change())
                .await
                .map(|()| ExitCode::SUCCESS)
        }
        (Some(Command::DeleteDocument { document_id }), _) => {
            delete(document_id).await.map(|()| ExitCode::SUCCESS)
        }
        (None, Some(path)) => ingest(path, &cli.given).await.map(|()| ExitCode::SUCCESS),
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
    Chapter(&'a Path, MediaLabels),
    Picture(&'a Path),
}

impl<'a> Source<'a> {
    /// Looks at the path, the note and the labels before anything is started, so a mistake costs
    /// nothing.
    fn of(path: &'a Path, given: &GivenPath) -> Result<Source<'a>> {
        if path.is_dir() {
            if given.note.is_some() {
                bail!(
                    "--note is for a picture, and {} is a folder; a chapter takes no note",
                    path.display()
                );
            }
            let category = given.category.unwrap_or_default();
            Ok(Source::Chapter(path, given.labels.of(category)))
        } else if path.is_file() {
            if given.category.is_some() || !given.labels.is_empty() {
                bail!(
                    "--category, --author and --tag label a media, and the picture {} belongs to no media; give its tags with --doc-tag",
                    path.display()
                );
            }
            Ok(Source::Picture(path))
        } else {
            bail!(
                "{} is not there; give a converted chapter folder, such as content/<media>/chapter-1, or a PNG or JPEG file",
                path.display()
            )
        }
    }
}

async fn ingest(path: &Path, given: &GivenPath) -> Result<()> {
    let source = Source::of(path, given)?;
    let config = Config::load().context("could not read the settings")?;
    // The models are set up before a picture is converted, so a missing key for the embedder
    // fails before the model that reads the picture is paid for.
    let models = set_up_models(&config)?;
    let stores = connect_stores(&config).await?;
    let summary: IngestSummary = match source {
        Source::Chapter(folder, new_media) => {
            let chapter = ChapterFolder {
                folder,
                new_media: &new_media,
            };
            ingest_chapter(chapter, &models, &stores)
                .await
                .with_context(|| format!("could not ingest {}", folder.display()))?
        }
        Source::Picture(file) => {
            let image = ocr::convert_image(file, &config.content_folder)
                .await
                .with_context(|| format!("could not convert {}", file.display()))?;
            ingest_image(
                &LoneImage {
                    image: &image,
                    note: given.note.as_deref(),
                },
                &models,
                &stores,
            )
            .await
            .with_context(|| format!("could not ingest {}", file.display()))?
        }
    };
    println!("{summary}");
    write_document_tags(summary.doc_id, &given.document.change(), &stores).await
}

async fn convert_and_ingest(given: &GivenPdf) -> Result<()> {
    let pdf = given.pdf.as_path();
    let (media_title, flag) = given.media.title_and_category()?;
    let config = Config::load().context("could not read the settings")?;
    if !pdf.is_file() {
        bail!("{} is not there; give the pdf to ingest", pdf.display());
    }
    // The models are set up before any page is converted, so a missing key for the embedder
    // fails before the first page is paid for.
    let models = set_up_models(&config)?;
    let stores = connect_stores(&config).await?;
    // A media that the library has keeps its own category, which says how the PDF is named, so
    // the PDF is named once the graph can be read. This still comes before anything is paid for.
    let unnamed = UnnamedPdf {
        media_title,
        category: Some(flag),
        document_title: given.title.as_deref(),
        pdf,
    };
    let named = unnamed
        .named(&stores.graph)
        .await
        .map_err(|error| naming_failure(error, &unnamed))?;
    if named.category != flag {
        eprintln!(
            "the library has {media_title:?} in the category {}, so this pdf is named and labelled by that category, not by --{flag}",
            named.category
        );
    }
    let new_media = given.labels.of(named.category);
    let job = ChapterJob::new(named.document, pdf, &config.content_folder)
        .with_context(|| format!("could not set up the conversion of {}", pdf.display()))?;
    // The key goes to the converter as a value, because the settings never enter the process
    // environment.
    let jev_api_key = config.jev_api_key.as_ref().map(ApiKey::expose);
    let outcome = ingest_pdf(
        ChapterPdf {
            job: &job,
            new_media: &new_media,
            convert:
                async |job: &ChapterJob, on_page: &mut (dyn FnMut(PageProgress) + Send + '_)| {
                    ocr::convert_chapter_with_jev_key(job, jev_api_key, on_page).await
                },
            on_step: print_step,
        },
        &models,
        &stores,
    )
    .await
    .with_context(|| format!("could not ingest {}", pdf.display()))?;
    println!("{outcome}");
    write_document_tags(outcome.doc_id(), &given.document.change(), &stores).await
}

/// Only a book refuses a name, so a refusal says "the library has" when the flag was not
/// `--book`: the book is then the category that the library has.
fn naming_failure(error: NamePdfError, unnamed: &UnnamedPdf) -> anyhow::Error {
    let UnnamedPdf {
        media_title,
        category: flag,
        pdf,
        ..
    } = *unnamed;
    match error {
        NamePdfError::ReadMedia(source) => {
            anyhow::Error::new(source).context("could not read the media that the graph holds")
        }
        NamePdfError::TitleForABook => anyhow!(
            "the library has {media_title:?} in the category book, and a book's chapter is named by its file name; leave out --title"
        ),
        NamePdfError::ChapterFileName(source) => {
            let fix = if flag == Some(Category::Book) {
                format!(
                    "{} cannot be ingested as a book chapter; rename it to chapter-<number>-<name>.pdf, or, when the library does not have {media_title:?} yet, give --paper or --other, whose pdf can have any name",
                    pdf.display()
                )
            } else {
                format!(
                    "the library has {media_title:?} in the category book, so its pdf must be named chapter-<number>-<name>.pdf; rename {}",
                    pdf.display()
                )
            };
            anyhow::Error::new(source).context(fix)
        }
    }
}

/// Own tags come after the ingest, so a run that stops in the ingest tags nothing, and the same
/// command again finishes both.
async fn write_document_tags(
    document: DocId,
    change: &TagChange,
    stores: &Stores<FalkorGraph>,
) -> Result<()> {
    if change.is_empty() {
        return Ok(());
    }
    write_tags(document, change, stores).await
}

async fn tag(document: DocId, change: &TagChange) -> Result<()> {
    let config = Config::load().context("could not read the settings")?;
    let stores = connect_stores(&config).await?;
    write_tags(document, change, &stores).await
}

async fn write_tags(
    document: DocId,
    change: &TagChange,
    stores: &Stores<FalkorGraph>,
) -> Result<()> {
    let labelled = relabel_document_tags(document, change, stores)
        .await
        .with_context(|| format!("could not tag the document {document}"))?;
    println!("{labelled}");
    Ok(())
}

async fn relabel_stored_media(title: &str, change: &MediaChange) -> Result<()> {
    if change.is_empty() {
        bail!(
            "give at least one of --category, --author, --no-authors, --tag and --no-tags to change the media {title:?}"
        );
    }
    let config = Config::load().context("could not read the settings")?;
    let stores = connect_stores(&config).await?;
    let relabelled = relabel_media(title, change, &stores)
        .await
        .with_context(|| format!("could not label the media {title:?}"))?;
    println!("{relabelled}");
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
