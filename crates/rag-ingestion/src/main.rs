//! The `rag-ingest` command: checks that the services are ready, ingests a converted chapter or a
//! picture, converts and ingests a PDF, relabels a stored media or document, or deletes a
//! document.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::builder::NonEmptyStringValueParser;
use clap::{Args, CommandFactory, Parser, Subcommand};
use graph::FalkorGraph;
use ocr::{ChapterJob, MediaDocument};
use rag_core::{
    ApiKey, Category, ClaudeCli, ConceptStore, Config, DocId, GeminiEmbedder, ItemStore,
    MediaLabels, Tag, author_list,
};
use rag_ingestion::{
    ChapterFolder, ChapterPdf, ConceptExtractor, EXTRACTION_MODEL, IngestSummary, LoneImage,
    MediaChange, Models, Stores, TagChange, delete_document, document_name, health, ingest_chapter,
    ingest_image, ingest_pdf, relabel_document_tags, relabel_media,
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

    #[command(flatten)]
    given: GivenPath,
}

// The structs below have no doc comment, because clap would print one as the first line of
// `rag-ingest --help`.
#[derive(Args)]
struct GivenPath {
    // This help text is an attribute and not a doc comment, because rustdoc takes `<media>` for
    // an HTML tag that is never closed.
    #[arg(
        help = "A converted chapter folder, such as content/<media>/chapter-1, or a PNG or JPEG file that stands alone, such as a chart"
    )]
    path: Option<PathBuf>,

    #[arg(
        long,
        help = "What you know about the picture. It is added to the explanation of the picture"
    )]
    note: Option<String>,

    /// The category of the chapter's media: book (the default), paper or other. It is used only
    /// when the library does not have this media yet. Not for a picture
    #[arg(long)]
    category: Option<Category>,

    #[command(flatten)]
    labels: NewMediaLabels,

    #[command(flatten)]
    document: DocumentTags,
}

#[derive(Args)]
struct GivenPdf {
    #[command(flatten)]
    media: PdfMedia,

    /// The title of this document, for a paper or another media. It defaults to the title of the
    /// media
    #[arg(long, conflicts_with = "book")]
    title: Option<String>,

    // This help text is an attribute and not a doc comment, because rustdoc takes `<number>` for
    // an HTML tag that is never closed.
    #[arg(help = "The PDF; a book chapter's PDF must be named chapter-<number>-<name>.pdf")]
    pdf: PathBuf,

    #[command(flatten)]
    labels: NewMediaLabels,

    #[command(flatten)]
    document: DocumentTags,
}

#[derive(Args)]
struct NewMediaLabels {
    /// An author of the media. Repeat it for more. It is used only when the library does not
    /// have this media yet: use `rag-ingest media` to change a stored media
    #[arg(long = "author", value_name = "AUTHOR", value_parser = NonEmptyStringValueParser::new())]
    authors: Vec<String>,

    /// A tag of the media, such as options. Repeat it for more. It is used only when the library
    /// does not have this media yet: use `rag-ingest media` to change a stored media
    #[arg(long = "tag", value_name = "TAG")]
    tags: Vec<Tag>,
}

impl NewMediaLabels {
    fn is_empty(&self) -> bool {
        self.authors.is_empty() && self.tags.is_empty()
    }

    fn of(&self, category: Category) -> MediaLabels {
        MediaLabels {
            category,
            authors: author_list(&self.authors),
            tags: self.tags.iter().cloned().collect(),
        }
    }
}

#[derive(Args)]
struct DocumentTags {
    /// A tag of this document only. Repeat it to add more. An ingest never removes a tag: use
    /// `rag-ingest tag --remove` for that
    #[arg(long = "doc-tag", value_name = "TAG")]
    doc_tags: Vec<Tag>,
}

impl DocumentTags {
    fn change(&self) -> TagChange {
        TagChange {
            add: self.doc_tags.clone(),
            remove: Vec::new(),
        }
    }
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct PdfMedia {
    // These help texts are attributes and not doc comments, because rustdoc takes `<number>` for
    // an HTML tag that is never closed.
    #[arg(
        long,
        value_name = "TITLE",
        help = "The title of the book the PDF is a chapter of. The PDF must be named chapter-<number>-<name>.pdf"
    )]
    book: Option<String>,

    /// The title of the paper. The PDF can have any name
    #[arg(long, value_name = "TITLE")]
    paper: Option<String>,

    /// The title of a media that is neither a book nor a paper. The PDF can have any name
    #[arg(long, value_name = "TITLE")]
    other: Option<String>,
}

impl PdfMedia {
    fn title_and_category(&self) -> Result<(&str, Category)> {
        let given = [
            (&self.book, Category::Book),
            (&self.paper, Category::Paper),
            (&self.other, Category::Other),
        ];
        given
            .into_iter()
            .find_map(|(title, category)| title.as_deref().map(|title| (title, category)))
            .context("give one of --book, --paper and --other")
    }
}

#[derive(Subcommand)]
enum Command {
    /// Check that Qdrant and FalkorDB answer and that the claude CLI is signed in
    Health,

    /// Convert one PDF of a book, a paper or another media and ingest it, in one run that needs
    /// no one
    Pdf(GivenPdf),

    /// Change the own tags of a stored document, in both stores, with no embedding and no model
    Tag {
        /// The document id that an ingest prints, such as 5f3c2a1e-9b04-5d6e-8a17-2c4b7e90f1d3
        document_id: DocId,

        /// A tag to add. Repeat it to add more
        #[arg(long, value_name = "TAG")]
        add: Vec<Tag>,

        /// A tag to take away. Repeat it to take away more
        #[arg(long, value_name = "TAG")]
        remove: Vec<Tag>,
    },

    /// Change the category, the authors or the tags of a stored media, and of every document of
    /// it, in both stores, with no embedding and no model
    Media {
        /// The title of the media, whatever its capitals
        title: String,

        /// The new category: book, paper or other
        #[arg(long)]
        category: Option<Category>,

        /// An author of the media. Repeat it for more. Given at all, the list replaces the
        /// authors the media has
        #[arg(long = "author", value_name = "AUTHOR", value_parser = NonEmptyStringValueParser::new())]
        authors: Vec<String>,

        /// A tag of the media. Repeat it for more. Given at all, the list replaces the tags the
        /// media has
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<Tag>,
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
            convert: async |job: &ChapterJob| {
                ocr::convert_chapter_with_jev_key(job, jev_api_key).await
            },
        },
        &models,
        &stores,
    )
    .await
    .with_context(|| format!("could not ingest {}", pdf.display()))?;
    println!("{outcome}");
    write_document_tags(outcome.doc_id(), &given.document.change(), &stores).await
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
