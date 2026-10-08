//! The `rag-ingest` command: checks that the services are ready, ingests a converted chapter or a
//! picture, converts and ingests a PDF, relabels a stored media or document, or deletes a
//! document.

mod cli;

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser};
use graph::FalkorGraph;
use ocr::{ChapterJob, MediaDocument, PageProgress};
use rag_core::{
    ApiKey, Category, ClaudeCli, ConceptStore, Config, DocId, GeminiEmbedder, ItemStore,
    MediaLabels, Tag,
};
use rag_ingestion::{
    ChapterFolder, ChapterPdf, ConceptExtractor, EXTRACTION_MODEL, IngestStep, IngestSummary,
    LoneImage, MediaChange, Models, Stores, TagChange, delete_document, document_name, health,
    ingest_chapter, ingest_image, ingest_pdf, relabel_document_tags, relabel_media,
};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::cli::{Cli, Command, GivenPath, GivenPdf};

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
        (
            Some(Command::Media {
                title,
                category,
                authors,
                tags,
            }),
            _,
        ) => relabel_stored_media(&title, &media_change(category, authors, tags))
            .await
            .map(|()| ExitCode::SUCCESS),
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
    let (media_title, category) = given.media.title_and_category()?;
    let new_media = given.labels.of(category);
    let config = Config::load().context("could not read the settings")?;
    // These checks come before anything is set up, so a mistake costs nothing.
    let title = given
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(media_title);
    let name = document_name(category, title, pdf).with_context(|| {
        format!(
            "{} cannot be ingested as a book chapter; rename it, or give a paper or other media with --paper or --other, whose pdf can have any name",
            pdf.display()
        )
    })?;
    let document = MediaDocument {
        media_title: media_title.to_owned(),
        name,
    };
    let job = ChapterJob::new(document, pdf, &config.content_folder)
        .with_context(|| format!("could not set up the conversion of {}", pdf.display()))?;
    if !pdf.is_file() {
        bail!("{} is not there; give the pdf to ingest", pdf.display());
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

/// The steps go to standard error, so standard output holds only the summary at the end.
fn print_step(step: IngestStep) {
    if let Some(line) = progress_line(&step) {
        eprintln!("{line}");
    }
}

/// One line for each page, and one for each later stage when it starts, never one for each item.
fn progress_line(step: &IngestStep) -> Option<String> {
    match *step {
        IngestStep::Converting(PageProgress::Pages { total, done_before }) => Some(format!(
            "converting {} of {total} pages ({done_before} already done)",
            total - done_before
        )),
        IngestStep::Converting(PageProgress::PageDone { position, cost_usd }) => {
            Some(format!("page {position} converted (${cost_usd:.2})"))
        }
        IngestStep::Converting(PageProgress::PageFailed { position }) => {
            Some(format!("page {position} failed"))
        }
        IngestStep::WritingGraph => Some("writing the graph".to_owned()),
        IngestStep::Embedding { items } => Some(format!("embedding {items} items")),
        IngestStep::Storing => Some("storing the items".to_owned()),
        IngestStep::ReadingConcepts { done: 1, total } => {
            Some(format!("reading the concepts of {total} items"))
        }
        IngestStep::LinkingConcepts { done: 1, total } => {
            Some(format!("linking the concepts of {total} items"))
        }
        IngestStep::ReadingConcepts { .. } | IngestStep::LinkingConcepts { .. } => None,
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

// SMELL: a flag that is not given keeps that label, so the authors or the tags of a media cannot
// be emptied from here.
fn media_change(category: Option<Category>, authors: Vec<String>, tags: Vec<Tag>) -> MediaChange {
    MediaChange {
        category,
        authors: (!authors.is_empty()).then_some(authors),
        tags: (!tags.is_empty()).then(|| tags.into_iter().collect()),
    }
}

async fn relabel_stored_media(title: &str, change: &MediaChange) -> Result<()> {
    if change.is_empty() {
        bail!("give at least one of --category, --author and --tag to change the media {title:?}");
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

#[cfg(test)]
mod tests {
    use ocr::PageProgress;
    use rag_ingestion::IngestStep;

    use super::progress_line;

    #[test]
    fn progress_lines_name_each_page_and_each_stage_once() {
        let pages = IngestStep::Converting(PageProgress::Pages {
            total: 12,
            done_before: 4,
        });
        let page = IngestStep::Converting(PageProgress::PageDone {
            position: 5,
            cost_usd: 0.137,
        });
        let first_read = IngestStep::ReadingConcepts { done: 1, total: 44 };
        let second_read = IngestStep::ReadingConcepts { done: 2, total: 44 };

        assert_eq!(
            progress_line(&pages).as_deref(),
            Some("converting 8 of 12 pages (4 already done)")
        );
        assert_eq!(
            progress_line(&page).as_deref(),
            Some("page 5 converted ($0.14)")
        );
        assert_eq!(
            progress_line(&first_read).as_deref(),
            Some("reading the concepts of 44 items")
        );
        assert_eq!(progress_line(&second_read), None);
    }
}
