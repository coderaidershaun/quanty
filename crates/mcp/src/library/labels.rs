//! The `edit_media` and `edit_document_tags` tools: change the labels of a stored media, or the
//! own tags of a stored document, in both stores. Nothing is embedded and no model is asked.

use std::collections::BTreeSet;

use graph::FalkorGraph;
use rag_core::{Category, ConceptStore, Config, ItemStore, Tag};
use rag_ingestion::{
    MediaChange, MediaRelabelled, Relabelled, Stores, TagChange, relabel_document_tags,
    relabel_media,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::LibraryError;
use super::chapter::parse_document_id;
use crate::args::{non_blank, parse_optional, parse_tags};

/// What an agent gives `edit_media`. A label that is left out stays as it is.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct EditMediaArgs {
    /// Media title.
    media: String,
    /// `book`, `paper` or `other`.
    category: Option<String>,
    /// Replaces authors; `[]` clears.
    authors: Option<Vec<String>>,
    /// Replaces tags; `[]` clears.
    tags: Option<Vec<String>>,
}

/// What an agent gives `edit_document_tags`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct EditDocumentTagsArgs {
    /// From `list_documents`.
    document_id: String,
    /// Tags to add.
    add: Option<Vec<String>>,
    /// Tags to remove, after `add`.
    remove: Option<Vec<String>>,
}

/// The labels a media has after `edit_media`.
#[derive(Serialize, JsonSchema)]
pub(crate) struct MediaLabelsView {
    /// The title as the library has it, which may differ in capitals from the one given.
    media: String,
    /// `book`, `paper` or `other`.
    category: String,
    authors: Vec<String>,
    tags: Vec<String>,
    /// How many documents of the media now carry these labels.
    documents: usize,
}

impl From<MediaRelabelled> for MediaLabelsView {
    fn from(relabelled: MediaRelabelled) -> MediaLabelsView {
        MediaLabelsView {
            media: relabelled.title,
            category: relabelled.labels.category.as_str().to_owned(),
            authors: relabelled.labels.authors,
            tags: tag_names(&relabelled.labels.tags),
            documents: relabelled.documents,
        }
    }
}

/// The tags a document has after `edit_document_tags`.
#[derive(Serialize, JsonSchema)]
pub(crate) struct DocumentTagsView {
    document_id: String,
    /// The tags of this document only.
    tags: Vec<String>,
    /// The tags of its media, which `edit_document_tags` leaves as they are.
    media_tags: Vec<String>,
}

impl From<Relabelled> for DocumentTagsView {
    fn from(relabelled: Relabelled) -> DocumentTagsView {
        DocumentTagsView {
            document_id: relabelled.doc_id.to_string(),
            tags: tag_names(&relabelled.labels.tags),
            media_tags: tag_names(&relabelled.labels.media_tags),
        }
    }
}

fn tag_names(tags: &BTreeSet<Tag>) -> Vec<String> {
    tags.iter().map(ToString::to_string).collect()
}

/// Gives the stored media of that title the labels that were given. It checks the arguments
/// before it connects to any store.
///
/// # Errors
/// An argument that is not valid, nothing to change, a store that is not ready, or a media that
/// is not stored.
pub(crate) async fn edit_media(
    config: &Config,
    args: EditMediaArgs,
) -> Result<MediaLabelsView, LibraryError> {
    let media = non_blank(Some(args.media)).ok_or(LibraryError::BlankMedia)?;
    let category = parse_optional::<Category>(args.category).map_err(LibraryError::Category)?;
    let tags = args
        .tags
        .map(|tags| parse_tags::<BTreeSet<Tag>>(Some(tags)))
        .transpose()
        .map_err(LibraryError::MediaTag)?;
    let change = MediaChange {
        category,
        authors: args.authors,
        tags,
    };
    if change.is_empty() {
        return Err(LibraryError::NothingToChange);
    }
    let stores = connect_stores(config).await?;
    Ok(relabel_media(&media, &change, &stores).await?.into())
}

/// Adds tags to the stored document and takes tags away from it. It checks the arguments before
/// it connects to any store.
///
/// # Errors
/// An argument that is not valid, nothing to change, a store that is not ready, or a document
/// that is not stored.
pub(crate) async fn edit_document_tags(
    config: &Config,
    args: EditDocumentTagsArgs,
) -> Result<DocumentTagsView, LibraryError> {
    let id = parse_document_id(&args.document_id)?;
    let change = TagChange {
        add: parse_tags(args.add).map_err(LibraryError::DocumentTag)?,
        remove: parse_tags(args.remove).map_err(LibraryError::DocumentTag)?,
    };
    if change.is_empty() {
        return Err(LibraryError::NoTagChange);
    }
    let stores = connect_stores(config).await?;
    Ok(relabel_document_tags(id, &change, &stores).await?.into())
}

/// The three stores, the graph first like every other tool.
async fn connect_stores(config: &Config) -> Result<Stores, LibraryError> {
    let graph = FalkorGraph::connect(config).await?;
    let items = ItemStore::connect(config).map_err(LibraryError::ItemStore)?;
    let concepts = ConceptStore::connect(config).map_err(LibraryError::ConceptStore)?;
    Ok(Stores {
        items,
        graph,
        concepts,
    })
}
