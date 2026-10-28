//! The `list_media` tool: the media of the library, such as books and papers, with their labels
//! and how many documents, pages and items each holds.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use graph::{DocumentRecord, FalkorGraph, MediaNode};
use ocr::content::INSIGHTS_FILE;
use ocr::{Catalogue, ChapterEntry};
use rag_core::{Category, Config, DocId, Tag, is_same_name};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::LibraryError;
use super::chapter::document_of;
use super::discovery::Discovery;
use crate::args::{non_blank, parse_optional};

/// What an agent gives `list_media`. Every part is optional.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ListMediaArgs {
    /// Only media of this category: `book`, `paper` or `other`.
    category: Option<String>,
    /// Only media by this author, whatever the capitals.
    author: Option<String>,
    /// Only media with this tag, whatever the capitals.
    tag: Option<String>,
    /// `summary` (default) or `names`.
    detail: Option<MediaDetail>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MediaDetail {
    /// Labels and counts.
    #[default]
    Summary,
    /// The titles only.
    Names,
}

/// The media of the library, sorted by title.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Media {
    media: Vec<MediaRow>,
    /// Only with `detail: names`.
    #[serde(flatten)]
    discovery: Option<Discovery>,
}

#[derive(Serialize, JsonSchema)]
struct MediaRow {
    /// Give it as `media` to `list_documents`.
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    /// How many stored documents it has.
    #[serde(skip_serializing_if = "Option::is_none")]
    documents: Option<usize>,
    /// The pages of its converted chapters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<u32>,
    /// The stored items of its documents, of every kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<u64>,
    /// How many of its documents have saved insights.
    #[serde(skip_serializing_if = "Option::is_none")]
    with_insights: Option<usize>,
}

/// What is known of one media while the rows are made.
struct Entry {
    title: String,
    category: Option<Category>,
    authors: Vec<String>,
    tags: BTreeSet<Tag>,
    documents: usize,
    items: u64,
    with_insights: usize,
}

impl Entry {
    fn fits(&self, category: Option<Category>, author: Option<&str>, tag: Option<&Tag>) -> bool {
        category.is_none_or(|wanted| self.category == Some(wanted))
            && author
                .is_none_or(|wanted| self.authors.iter().any(|have| is_same_name(have, wanted)))
            && tag.is_none_or(|wanted| self.tags.contains(wanted))
    }
}

/// The media that fit the filters. A media is a node of the graph, or only the title that some
/// documents carry. It reads the graph for the media and the documents, and the content folder
/// for the pages.
///
/// # Errors
/// A bad argument, the graph that is not ready, or a content folder that cannot be listed.
pub(crate) async fn list_media(
    config: &Config,
    args: ListMediaArgs,
) -> Result<Media, LibraryError> {
    let detail = args.detail.unwrap_or_default();
    let category = parse_optional::<Category>(args.category).map_err(LibraryError::Category)?;
    let author = non_blank(args.author);
    let tag = parse_optional::<Tag>(args.tag).map_err(LibraryError::FilterTag)?;
    let graph = FalkorGraph::connect(config).await?;
    let nodes = graph.media().await?;
    let records = graph.document_records().await?;
    let catalogue = Catalogue::read(&config.content_folder)?;
    let media = entries_of(nodes, records, &catalogue)
        .into_values()
        .filter(|entry| entry.fits(category, author.as_deref(), tag.as_ref()))
        .collect::<Vec<_>>();
    let discovery = (detail == MediaDetail::Names).then(|| {
        media.iter().fold(Discovery::new(), |discovery, entry| {
            let arguments = json!({ "media": entry.title, "detail": "names" });
            discovery.discovering(&entry.title, "list_documents", arguments)
        })
    });
    let rows = media
        .into_iter()
        .map(|entry| match detail {
            MediaDetail::Names => MediaRow::titled(entry.title),
            MediaDetail::Summary => MediaRow::summarised(entry, &catalogue),
        })
        .collect();
    Ok(Media {
        media: rows,
        discovery,
    })
}

/// Every media by its key: the media nodes, then the titles that only documents carry, each with
/// the counts of its documents.
fn entries_of(
    nodes: Vec<MediaNode>,
    records: Vec<DocumentRecord>,
    catalogue: &Catalogue,
) -> BTreeMap<String, Entry> {
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    for node in nodes {
        entries.insert(
            key_of(&node.title),
            Entry {
                title: node.title,
                category: Some(node.labels.category),
                authors: node.labels.authors,
                tags: node.labels.tags,
                documents: 0,
                items: 0,
                with_insights: 0,
            },
        );
    }
    let chapters: HashMap<DocId, &ChapterEntry> = catalogue
        .chapters
        .iter()
        .map(|entry| (document_of(entry), entry))
        .collect();
    for record in records {
        add_document(&mut entries, record, &chapters);
    }
    entries
}

/// Counts the document in its media, and makes the media when only the document names it.
fn add_document(
    entries: &mut BTreeMap<String, Entry>,
    record: DocumentRecord,
    chapters: &HashMap<DocId, &ChapterEntry>,
) {
    let DocumentRecord { node, items, .. } = record;
    let Some(title) = node.labels.media.clone() else {
        return;
    };
    let entry = entries.entry(key_of(&title)).or_insert_with(|| Entry {
        title,
        category: node.labels.category,
        authors: node.labels.authors.clone(),
        tags: node.labels.media_tags.clone(),
        documents: 0,
        items: 0,
        with_insights: 0,
    });
    entry.documents += 1;
    entry.items += items.chunks + items.formulas + items.figures + items.tables + items.code;
    let has_insights = chapters
        .get(&node.id)
        .is_some_and(|chapter| chapter.folder.join(INSIGHTS_FILE).is_file());
    entry.with_insights += usize::from(has_insights);
}

/// Two titles that differ in the capitals only are one media.
// SMELL: this repeats the rule of `is_same_name`, so the two must change together.
fn key_of(title: &str) -> String {
    title.trim().to_lowercase()
}

impl MediaRow {
    fn titled(title: String) -> MediaRow {
        MediaRow {
            title,
            category: None,
            authors: None,
            tags: None,
            documents: None,
            pages: None,
            items: None,
            with_insights: None,
        }
    }

    fn summarised(entry: Entry, catalogue: &Catalogue) -> MediaRow {
        let pages = catalogue
            .chapters
            .iter()
            .filter(|chapter| is_same_name(&chapter.index.media_title, &entry.title))
            .map(|chapter| chapter.index.page_count)
            .sum();
        MediaRow {
            category: entry.category.map(|category| category.as_str().to_owned()),
            authors: Some(entry.authors),
            tags: Some(entry.tags.iter().map(ToString::to_string).collect()),
            documents: Some(entry.documents),
            pages: Some(pages),
            items: Some(entry.items),
            with_insights: Some(entry.with_insights),
            ..MediaRow::titled(entry.title)
        }
    }
}
