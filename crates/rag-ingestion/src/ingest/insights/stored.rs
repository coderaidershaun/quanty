//! Which documents are stored already and where their converted folders are, for the command that
//! writes the insights of chapters that were ingested before insights were written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use graph::{DocumentNode, FalkorGraph, GraphError};
use ocr::{Catalogue, ChapterEntry, ChapterInsights, ContentError, gets_insights};
use rag_core::{DocId, is_same_name};

/// Which stored documents a command works on.
#[derive(Debug, Clone, Copy)]
pub enum ChapterChoice<'a> {
    Document(DocId),
    /// Every document of the media with this title, whatever its capitals.
    Media(&'a str),
    /// Every document that belongs to a media. A picture that stands alone belongs to none.
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChapter {
    pub doc_id: DocId,
    pub title: String,
    /// The finished converted folder of the document, or `None` when the content folder holds
    /// none.
    pub folder: Option<PathBuf>,
    /// Whether the folder holds an `insights.json` that can be read.
    pub has_insights: bool,
    /// Whether the document gets insights at all: a part of a book that is not a chapter gets
    /// none. A document with no folder has no name to judge by, so it counts as one that does.
    pub gets_insights: bool,
}

/// What a run that writes missing insights does for one chosen document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsightsNeed<'a> {
    /// The insights are missing: ask the model and save them in this converted folder.
    Write(&'a Path),
    /// The file is saved already, so nothing is asked.
    AlreadySaved,
    /// The content folder holds no finished converted chapter of the document.
    NoChapter,
    /// A part of a book that is not a chapter gets none.
    BookPart,
}

impl StoredChapter {
    /// What a run that writes missing insights does for this document: a part of a book first,
    /// then a document with no converted folder, then a file that is saved already.
    pub fn need(&self) -> InsightsNeed<'_> {
        if !self.gets_insights {
            return InsightsNeed::BookPart;
        }
        match (&self.folder, self.has_insights) {
            (None, _) => InsightsNeed::NoChapter,
            (Some(_), true) => InsightsNeed::AlreadySaved,
            (Some(folder), false) => InsightsNeed::Write(folder),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StoredChaptersError {
    #[error("could not read the stored documents from the graph")]
    Graph(#[from] GraphError),

    #[error("could not list the converted chapters in the content folder")]
    Catalogue(#[from] ContentError),

    #[error(
        "the graph holds no document with the id {id}; use the id that an ingest prints as \"document id\""
    )]
    NotStored { id: DocId },

    #[error(
        "the graph holds no document of a media titled {title:?}; use the title that the media was ingested with"
    )]
    NoDocumentOfMedia { title: String },
}

/// The chosen documents with the folder and the saved file of each. It asks no model and writes
/// nothing.
///
/// # Errors
/// - [`StoredChaptersError::Graph`] and [`StoredChaptersError::Catalogue`] when the graph or the
///   content folder cannot be read
/// - [`StoredChaptersError::NotStored`] when no document has the chosen id
/// - [`StoredChaptersError::NoDocumentOfMedia`] when the chosen media has no document
pub async fn stored_chapters(
    choice: ChapterChoice<'_>,
    graph: &FalkorGraph,
    content_folder: &Path,
) -> Result<Vec<StoredChapter>, StoredChaptersError> {
    let mut documents = chosen_documents(choice, graph.documents().await?)?;
    documents.sort_by(|one, other| (&one.title, one.id).cmp(&(&other.title, other.id)));
    // The graph's own record of a chapter's folder is never set by an ingest, so the folders are
    // found by listing the content folder.
    let converted: HashMap<DocId, ChapterEntry> = Catalogue::read(content_folder)?
        .chapters
        .into_iter()
        .filter(|entry| entry.index.finished)
        .map(|entry| (DocId::from_source_sha256(&entry.index.source_sha256), entry))
        .collect();
    Ok(documents
        .into_iter()
        .map(|document| {
            let chapter = converted.get(&document.id);
            let folder = chapter.map(|chapter| chapter.folder.clone());
            let has_insights = folder
                .as_deref()
                .is_some_and(|folder| matches!(ChapterInsights::read(folder), Ok(Some(_))));
            StoredChapter {
                doc_id: document.id,
                title: document.title,
                folder,
                has_insights,
                gets_insights: chapter.is_none_or(|chapter| {
                    gets_insights(&chapter.index.name, document.labels.category)
                }),
            }
        })
        .collect())
}

fn chosen_documents(
    choice: ChapterChoice<'_>,
    stored: Vec<DocumentNode>,
) -> Result<Vec<DocumentNode>, StoredChaptersError> {
    match choice {
        ChapterChoice::Document(id) => {
            let found: Vec<DocumentNode> =
                stored.into_iter().filter(|node| node.id == id).collect();
            if found.is_empty() {
                return Err(StoredChaptersError::NotStored { id });
            }
            Ok(found)
        }
        ChapterChoice::Media(title) => {
            let found: Vec<DocumentNode> = stored
                .into_iter()
                .filter(|node| {
                    let media = node.labels.media.as_deref();
                    media.is_some_and(|media| is_same_name(media, title))
                })
                .collect();
            if found.is_empty() {
                return Err(StoredChaptersError::NoDocumentOfMedia {
                    title: title.to_owned(),
                });
            }
            Ok(found)
        }
        ChapterChoice::All => Ok(stored
            .into_iter()
            .filter(|node| node.labels.media.is_some())
            .collect()),
    }
}
