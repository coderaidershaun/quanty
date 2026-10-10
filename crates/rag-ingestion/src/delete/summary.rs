//! What a delete reports to the person who ran it.

use std::fmt;
use std::path::PathBuf;

use rag_core::DocId;

/// What a delete of one document removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDeleteSummary {
    pub doc_id: DocId,
    pub collection: String,
    pub points_removed: u64,
    /// The document node and its item nodes.
    pub nodes_removed: u64,
    /// The converted folders of the document that were removed from the content folder. Empty
    /// when the content folder held none. Each path is the content folder as it was given, joined
    /// with the names that the listing of that folder gave. It is never made absolute, so it is
    /// equal to the folder that the same listing gives for the document.
    pub folders_removed: Vec<PathBuf>,
    /// The copies of its PDF that were removed from the uploads folder. Each path starts with the
    /// content folder as it was given.
    pub uploads_removed: Vec<PathBuf>,
}

impl fmt::Display for DocumentDeleteSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines = vec![
            format!("document id: {}", self.doc_id),
            format!(
                "points removed from collection {}: {}",
                self.collection, self.points_removed
            ),
            format!(
                "nodes removed from the graph: {} (the document and its items)",
                self.nodes_removed
            ),
        ];
        lines.extend(path_lines(
            "converted folder removed",
            &self.folders_removed,
        ));
        lines.extend(path_lines("uploaded pdf removed", &self.uploads_removed));
        write!(formatter, "{}", lines.join("\n"))
    }
}

/// One line for each path, or one line that says none.
fn path_lines<'a>(label: &str, paths: impl IntoIterator<Item = &'a PathBuf>) -> Vec<String> {
    let lines: Vec<String> = paths
        .into_iter()
        .map(|path| format!("{label}: {}", path.display()))
        .collect();
    if lines.is_empty() {
        vec![format!("{label}: none")]
    } else {
        lines
    }
}

/// What a delete of a whole media removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDeleteSummary {
    /// As it was asked for.
    pub title: String,
    pub collection: String,
    /// One summary for each document of the media, in the order of their ids.
    pub documents: Vec<DocumentDeleteSummary>,
    /// The folder of the media and its folder of uploads, each only when it was there and empty
    /// after the documents went. Each path starts with the content folder as it was given.
    pub media_folders_removed: Vec<PathBuf>,
    /// `false` when the graph held no media of that title: documents can carry a title that no
    /// media has.
    pub media_node_removed: bool,
}

impl fmt::Display for MediaDeleteSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let documents = &self.documents;
        let points: u64 = documents
            .iter()
            .map(|document| document.points_removed)
            .sum();
        let nodes: u64 = documents
            .iter()
            .map(|document| document.nodes_removed)
            .sum();
        let mut lines = vec![
            format!("media: {:?}", self.title),
            format!("documents removed: {}", documents.len()),
        ];
        lines.extend(
            documents
                .iter()
                .map(|document| format!("document id: {}", document.doc_id)),
        );
        lines.push(format!(
            "points removed from collection {}: {points}",
            self.collection
        ));
        lines.push(format!(
            "nodes removed from the graph: {nodes} (the documents and their items)"
        ));
        let folders = documents
            .iter()
            .flat_map(|document| &document.folders_removed);
        lines.extend(path_lines("converted folder removed", folders));
        let uploads = documents
            .iter()
            .flat_map(|document| &document.uploads_removed);
        lines.extend(path_lines("uploaded pdf removed", uploads));
        lines.extend(path_lines(
            "media folder removed",
            &self.media_folders_removed,
        ));
        let media_removed = if self.media_node_removed { "yes" } else { "no" };
        lines.push(format!("media removed from the graph: {media_removed}"));
        write!(formatter, "{}", lines.join("\n"))
    }
}
