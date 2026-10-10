//! Removes a document, or a whole media, from everything quanty holds of it: its points in Qdrant,
//! its nodes in the graph, and its folders in the content folder.

mod files;
mod summary;

use std::path::{Path, PathBuf};

use graph::{GraphError, GraphStore, MediaNode};
use ocr::ContentError;
use rag_core::{DocId, StoreError, is_same_name};

use crate::stores::Stores;
use files::DocumentFiles;
pub use summary::{DocumentDeleteSummary, MediaDeleteSummary};

#[derive(thiserror::Error, Debug)]
pub enum DeleteError {
    #[error("could not remove the points of the document")]
    Store(#[from] StoreError),

    #[error("could not read or remove nodes in the graph")]
    Graph(#[from] GraphError),

    /// The content folder could not be listed, the lock file of a converted folder could not be
    /// made, or the copy of a PDF under `_uploads` could not be read.
    #[error("could not read what the content folder holds of the document")]
    Content(#[from] ContentError),

    #[error("could not remove {}", path.display())]
    Remove {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "the document is being converted in {} at this moment, so nothing of it was removed; wait for that run to end, then delete again",
        folder.display()
    )]
    Converting { folder: PathBuf },

    #[error(
        "nothing is stored under the document id {id} in the collection {collection} or in the graph, and the content folder {} holds no folder of it, so nothing was removed; use the id that an ingest prints as \"document id\"",
        content_folder.display()
    )]
    UnknownDocument {
        id: DocId,
        collection: String,
        content_folder: PathBuf,
    },

    #[error(
        "the graph holds no media titled {title:?} and no document of such a media, so nothing was removed; use the title that the media was saved or ingested with"
    )]
    UnknownMedia { title: String },
}

/// Removes everything quanty holds of one document: its points in Qdrant, its converted folder
/// in the content folder, the copy of its PDF under `_uploads` when that copy is this document's
/// PDF, and its nodes in the graph. A picture that stands alone goes with its folder under
/// `images`. Other documents stay whole. So do the concepts, which belong to no document, and the
/// media of the document with its folder, also when this was its last document: a media with no
/// document is kept, like one that was saved before its first document.
///
/// The steps run in this order:
/// - The content folder is listed, and each converted folder of the document is taken. A document
///   that is being converted is refused here, before anything is removed, so a store that the
///   delete never reached stays whole. Only a conversion is seen: an ingest that goes on after
///   its conversion is not refused, so a caller must not delete a document while it is ingested.
/// - The points go before the nodes, so that no point is ever left without its node. An ingest
///   keeps the same rule.
/// - The upload copy goes before its folder, because the `chapter.json` in the folder is the only
///   thing that names the copy. A run that stops between the two leaves the folder, and the next
///   run finds it.
/// - The nodes go last. While anything of the document is left, the graph still lists the
///   document under its media, so the Library still shows it and a second delete finishes the job.
///
/// A run that stops between two steps leaves what the next run finishes: the folder, the points
/// and the nodes are each found without the others. One case is not found again. A run that is cut
/// short inside the removal of a folder, because the process is killed or the disk fails, can
/// leave a part of the folder without its index file, which is `chapter.json`, or `image.json`
/// for a picture. The listing knows a folder only by that file, so a person must remove what is
/// left of the folder by hand.
///
/// The files are found only by listing `content_folder`, never by a path that a store names.
///
/// # Errors
/// - [`DeleteError::Converting`] when a run is converting the document at this moment. Nothing
///   was removed.
/// - [`DeleteError::Content`] and [`DeleteError::Remove`] when the content folder cannot be read
///   or a file cannot be removed. The points can be gone by then, and the same call again
///   removes the rest.
/// - [`DeleteError::Store`] and [`DeleteError::Graph`] when a store fails. The first failure
///   stops the job, so when Qdrant fails the graph is not asked.
/// - [`DeleteError::UnknownDocument`] when neither store nor the content folder held anything of
///   the id
pub async fn delete_document<G: GraphStore>(
    id: DocId,
    content_folder: &Path,
    stores: &Stores<G>,
) -> Result<DocumentDeleteSummary, DeleteError> {
    let summary = remove_document(id, content_folder, stores).await?;
    if summary.points_removed == 0
        && summary.nodes_removed == 0
        && summary.folders_removed.is_empty()
    {
        return Err(DeleteError::UnknownDocument {
            id,
            collection: summary.collection,
            content_folder: content_folder.to_path_buf(),
        });
    }
    Ok(summary)
}

/// Removes a whole media by its title, which is compared whatever its capitals and the space at
/// its ends. Every document that carries the title goes as [`delete_document`] removes it, in
/// the order of the ids. Then the folder of the media under the content folder and its folder
/// under `_uploads` go, each only when nothing is left in it, and last the `Media` node goes.
///
/// Two titles that differ only in punctuation name one folder, so a folder that still holds a
/// document of the other title stays. A converted folder whose document is not in the graph is
/// not found, and the folder of its media stays too. A media with no document is deleted without
/// a call to Qdrant. Documents whose title no `Media` node carries are deleted too.
///
/// # Errors
/// - [`DeleteError::UnknownMedia`] when the graph holds no media with this title and no document
///   of such a media. Nothing was removed.
/// - Any error of [`delete_document`] but [`DeleteError::UnknownDocument`], for the first
///   document that fails. On any error the documents before it are already removed, in the order
///   of their ids, and the same call again goes on from there, so a caller must read the library
///   again after a failure.
/// - [`DeleteError::Graph`] and [`DeleteError::Remove`] when the graph fails or a media folder
///   cannot be removed
pub async fn delete_media<G: GraphStore>(
    title: &str,
    content_folder: &Path,
    stores: &Stores<G>,
) -> Result<MediaDeleteSummary, DeleteError> {
    let media_nodes: Vec<MediaNode> = stores
        .graph
        .media()
        .await?
        .into_iter()
        .filter(|node| is_same_name(&node.title, title))
        .collect();
    let document_ids: Vec<DocId> = stores
        .graph
        .documents()
        .await?
        .into_iter()
        .filter(|node| {
            let media = node.labels.media.as_deref();
            media.is_some_and(|media| is_same_name(media, title))
        })
        .map(|node| node.id)
        .collect();
    if media_nodes.is_empty() && document_ids.is_empty() {
        return Err(DeleteError::UnknownMedia {
            title: title.to_owned(),
        });
    }
    let mut documents = Vec::new();
    for id in document_ids {
        documents.push(remove_document(id, content_folder, stores).await?);
    }
    let media_folders_removed = files::remove_empty_media_folders(title, content_folder)?;
    let mut media_node_removed = false;
    for node in &media_nodes {
        media_node_removed |= stores.graph.delete_media(&node.title).await?;
    }
    Ok(MediaDeleteSummary {
        title: title.to_owned(),
        collection: stores.items.collection().to_owned(),
        documents,
        media_folders_removed,
        media_node_removed,
    })
}

/// The steps of a delete of one document, in the order that [`delete_document`] gives.
async fn remove_document<G: GraphStore>(
    id: DocId,
    content_folder: &Path,
    stores: &Stores<G>,
) -> Result<DocumentDeleteSummary, DeleteError> {
    let files = DocumentFiles::find(id, content_folder)?;
    let points_removed = stores.items.delete_document(id).await?;
    let removed = files.remove(content_folder)?;
    let nodes_removed = stores.graph.delete_document(id).await?;
    Ok(DocumentDeleteSummary {
        doc_id: id,
        collection: stores.items.collection().to_owned(),
        points_removed,
        nodes_removed,
        folders_removed: removed.folders,
        uploads_removed: removed.uploads,
    })
}
