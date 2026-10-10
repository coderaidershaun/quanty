//! The arguments of the `rag-ingest` command, as clap reads them.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Parser, Subcommand};
use rag_core::{Category, DocId, MediaLabels, Tag, author_list};
use rag_ingestion::{MediaChange, TagChange};

#[derive(Parser)]
#[command(name = "rag-ingest", args_conflicts_with_subcommands = true)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Option<Command>,

    #[command(flatten)]
    pub(super) given: GivenPath,
}

// The structs below have no doc comment, because clap would print one as the first line of
// `rag-ingest --help`.
#[derive(Args)]
pub(super) struct GivenPath {
    // This help text is an attribute and not a doc comment, because rustdoc takes `<media>` for
    // an HTML tag that is never closed.
    #[arg(
        help = "A converted chapter folder, such as content/<media>/chapter-1, or a PNG or JPEG file that stands alone, such as a chart"
    )]
    pub(super) path: Option<PathBuf>,

    #[arg(
        long,
        help = "What you know about the picture. It is added to the explanation of the picture"
    )]
    pub(super) note: Option<String>,

    /// The category of the chapter's media: book (the default), paper or other. It is used only
    /// when the library does not have this media yet. Not for a picture
    #[arg(long)]
    pub(super) category: Option<Category>,

    #[command(flatten)]
    pub(super) labels: NewMediaLabels,

    #[command(flatten)]
    pub(super) document: DocumentTags,
}

#[derive(Args)]
pub(super) struct GivenPdf {
    #[command(flatten)]
    pub(super) media: PdfMedia,

    /// The title of this document, for a paper or another media. It defaults to the title of the
    /// media
    #[arg(long, conflicts_with = "book")]
    pub(super) title: Option<String>,

    // This help text is an attribute and not a doc comment, because rustdoc takes `<number>` for
    // an HTML tag that is never closed.
    #[arg(help = "The PDF; a book chapter's PDF must be named chapter-<number>-<name>.pdf")]
    pub(super) pdf: PathBuf,

    #[command(flatten)]
    pub(super) labels: NewMediaLabels,

    #[command(flatten)]
    pub(super) document: DocumentTags,
}

#[derive(Args)]
pub(super) struct NewMediaLabels {
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
    pub(super) fn is_empty(&self) -> bool {
        self.authors.is_empty() && self.tags.is_empty()
    }

    pub(super) fn of(&self, category: Category) -> MediaLabels {
        MediaLabels {
            category,
            authors: author_list(&self.authors),
            tags: self.tags.iter().cloned().collect(),
        }
    }
}

#[derive(Args)]
pub(super) struct DocumentTags {
    /// A tag of this document only. Repeat it to add more. An ingest never removes a tag: use
    /// `rag-ingest tag --remove` for that
    #[arg(long = "doc-tag", value_name = "TAG")]
    doc_tags: Vec<Tag>,
}

impl DocumentTags {
    pub(super) fn change(&self) -> TagChange {
        TagChange {
            add: self.doc_tags.clone(),
            remove: Vec::new(),
        }
    }
}

#[derive(Args)]
pub(super) struct GivenMediaLabels {
    /// The new category: book, paper or other
    #[arg(long)]
    category: Option<Category>,

    /// An author of the media. Repeat it for more. Given at all, the list replaces the
    /// authors the media has
    #[arg(long = "author", value_name = "AUTHOR", value_parser = NonEmptyStringValueParser::new())]
    authors: Vec<String>,

    /// Take every author away from the media
    #[arg(long = "no-authors", conflicts_with = "authors")]
    clears_authors: bool,

    /// A tag of the media. Repeat it for more. Given at all, the list replaces the tags the
    /// media has
    #[arg(long = "tag", value_name = "TAG")]
    tags: Vec<Tag>,

    /// Take every tag away from the media
    #[arg(long = "no-tags", conflicts_with = "tags")]
    clears_tags: bool,
}

impl GivenMediaLabels {
    pub(super) fn change(&self) -> MediaChange {
        MediaChange {
            category: self.category,
            authors: replacing(&self.authors, self.clears_authors),
            tags: replacing(&self.tags, self.clears_tags).map(|tags| tags.into_iter().collect()),
        }
    }
}

/// A list that is given replaces the stored one, and so does the empty list of a `--no-` flag.
/// With neither, the stored list stays.
fn replacing<T: Clone>(given: &[T], clears: bool) -> Option<Vec<T>> {
    (clears || !given.is_empty()).then(|| given.to_vec())
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub(super) struct PdfMedia {
    // These help texts are attributes and not doc comments, because rustdoc takes `<number>` for
    // an HTML tag that is never closed.
    #[arg(
        long,
        value_name = "TITLE",
        help = "The title of the book the PDF is a chapter of. The PDF must be named chapter-<number>-<name>.pdf. A media the library already has keeps its own category"
    )]
    book: Option<String>,

    /// The title of the paper. The PDF can have any name. A media the library already has keeps
    /// its own category
    #[arg(long, value_name = "TITLE")]
    paper: Option<String>,

    /// The title of a media that is neither a book nor a paper. The PDF can have any name. A media
    /// the library already has keeps its own category
    #[arg(long, value_name = "TITLE")]
    other: Option<String>,
}

impl PdfMedia {
    pub(super) fn title_and_category(&self) -> Result<(&str, Category)> {
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
pub(super) enum Command {
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

        #[command(flatten)]
        labels: GivenMediaLabels,
    },

    /// Remove one document: its items from Qdrant and from the graph, its converted folder, and
    /// the uploaded copy of its PDF. It asks no question
    DeleteDocument {
        /// The document id that an ingest prints, such as 5f3c2a1e-9b04-5d6e-8a17-2c4b7e90f1d3
        document_id: DocId,
    },

    /// Remove a whole media: every document of it, as delete-document does, and then the media
    /// itself. It asks no question
    DeleteMedia {
        /// The title of the media, whatever its capitals
        title: String,
    },
}
