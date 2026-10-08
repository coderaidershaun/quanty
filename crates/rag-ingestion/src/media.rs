//! Changes the labels of a stored media, and with them the copy that every document of it and
//! every point of those documents carries, without embedding anything or asking a model. Also
//! says which category a PDF of a media is named and labelled by.

use std::collections::BTreeSet;
use std::fmt;

use graph::{GraphError, GraphStore, MediaNode};
use rag_core::{Category, MediaLabels, Tag, author_list, is_same_name};

use crate::labels::{RelabelError, tag_text};
use crate::stores::Stores;

/// The title is not here: it names the media and cannot change. A label that is `None` stays as
/// it is, and one that is given replaces the whole label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaChange {
    pub category: Option<Category>,
    /// Trimmed, with blank names and second copies of a name dropped, in the order given.
    pub authors: Option<Vec<String>>,
    pub tags: Option<BTreeSet<Tag>>,
}

impl MediaChange {
    pub fn is_empty(&self) -> bool {
        self.category.is_none() && self.authors.is_none() && self.tags.is_none()
    }

    fn apply_to(&self, labels: &mut MediaLabels) {
        if let Some(category) = self.category {
            labels.category = category;
        }
        if let Some(authors) = &self.authors {
            labels.authors = author_list(authors);
        }
        if let Some(tags) = &self.tags {
            labels.tags = tags.clone();
        }
    }
}

/// The labels that a media has after [`relabel_media`], and how many documents carry them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRelabelled {
    /// As the graph has it, which may differ in capitals from the title that was asked for.
    pub title: String,
    pub labels: MediaLabels,
    pub documents: usize,
}

impl fmt::Display for MediaRelabelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let authors = if self.labels.authors.is_empty() {
            "none".to_owned()
        } else {
            self.labels.authors.join(", ")
        };
        writeln!(
            formatter,
            "labels written to the media {:?} and to its {} documents in both stores",
            self.title, self.documents
        )?;
        writeln!(formatter, "category: {}", self.labels.category)?;
        writeln!(formatter, "authors: {authors}")?;
        write!(formatter, "tags: {}", tag_text(&self.labels.tags))
    }
}

/// Applies the change to the stored media of that title, whatever its capitals: the media node
/// first, then each document of the media and every point of that document. The own tags of a
/// document stay as they are. It takes no embedder and no model, so it cannot call one. Running
/// it again with the same change changes nothing, and a run that stopped half way is finished by
/// running it again.
///
/// # Errors
/// - [`RelabelError::UnknownMedia`] when the graph holds no media of that title
/// - [`RelabelError::Graph`] and [`RelabelError::Store`] when a store fails
pub async fn relabel_media<G: GraphStore>(
    title: &str,
    change: &MediaChange,
    stores: &Stores<G>,
) -> Result<MediaRelabelled, RelabelError> {
    let mut media =
        stored_media(title, &stores.graph)
            .await?
            .ok_or_else(|| RelabelError::UnknownMedia {
                title: title.to_owned(),
            })?;
    change.apply_to(&mut media.labels);
    stores.graph.update_media(&media).await?;
    let mut documents = 0;
    for mut node in stores.graph.documents().await? {
        let of_this_media = node
            .labels
            .media
            .as_deref()
            .is_some_and(|other| is_same_name(other, &media.title));
        if !of_this_media {
            continue;
        }
        node.labels.take_media(&media.title, &media.labels);
        stores.graph.upsert_document(&node).await?;
        stores
            .items
            .set_document_labels(node.id, &node.labels)
            .await?;
        documents += 1;
    }
    Ok(MediaRelabelled {
        title: media.title,
        labels: media.labels,
        documents,
    })
}

/// The category of the media titled `title`. It is the stored media's when the library has one,
/// whatever `given` says; else `given`; else a book. A PDF of the media is named by it, and a new
/// media is made with it.
///
/// # Errors
/// [`GraphError`] when the graph cannot list its media.
pub async fn media_category<G: GraphStore>(
    title: &str,
    given: Option<Category>,
    graph: &G,
) -> Result<Category, GraphError> {
    Ok(match stored_media(title, graph).await? {
        Some(stored) => stored.labels.category,
        None => given.unwrap_or_default(),
    })
}

/// The stored media of that title, or `media` itself after it is added to the graph. An ingest
/// uses it, so it never changes a media that is stored.
pub(crate) async fn stored_or_added<G: GraphStore>(
    media: MediaNode,
    stores: &Stores<G>,
) -> Result<MediaNode, GraphError> {
    if let Some(stored) = stored_media(&media.title, &stores.graph).await? {
        return Ok(stored);
    }
    stores.graph.add_media(&media).await?;
    Ok(media)
}

/// The media whose title is the same as `title`, whatever the capitals and the space at the ends.
async fn stored_media<G: GraphStore>(
    title: &str,
    graph: &G,
) -> Result<Option<MediaNode>, GraphError> {
    let media = graph.media().await?;
    Ok(media
        .into_iter()
        .find(|stored| is_same_name(&stored.title, title)))
}
