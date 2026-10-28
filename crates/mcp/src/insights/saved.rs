//! The `chapter_insights` tool: the insights, takeaways and cheat sheet that were written for one
//! document, read from the file in its converted folder.

use std::path::Path;

use graph::FalkorGraph;
use ocr::content::INSIGHTS_FILE;
use ocr::{ChapterEntry, ChapterInsights, DocumentName, gets_insights};
use rag_core::{Category, Config, DocId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::InsightsToolError;
use crate::args::{Budget, Scope, Truncation};
use crate::library::{DocumentHeader, LibraryError, find_chapter, parse_document_id};

/// What an agent gives `chapter_insights`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ChapterInsightsArgs {
    /// From `list_documents`.
    document_id: String,
    /// Text limit. Default 24000.
    pub(crate) max_chars: Option<u32>,
    /// From a `truncated` result.
    pub(crate) cursor: Option<String>,
}

/// The notes written for one document, or why there are none.
#[derive(Serialize, JsonSchema)]
pub(crate) struct InsightsView {
    #[serde(flatten)]
    document: DocumentHeader,
    state: SavedState,
    /// The file the notes are saved in. Only when `saved`.
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    /// The model that wrote them, as its agent names it. Only when `saved`.
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Which wording of the question asked for them. Only when `saved`.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_version: Option<String>,
    /// Short Markdown paragraphs, with formulas between `$` or `$$`. Empty unless `saved`.
    insights: Vec<InsightView>,
    /// One line of Markdown each. Empty unless `saved`.
    takeaways: Vec<String>,
    /// Markdown with short headed sections, formulas and small tables. Only when `saved`.
    #[serde(skip_serializing_if = "Option::is_none")]
    cheat_sheet: Option<String>,
    #[serde(flatten)]
    more: Truncation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SavedState {
    /// The notes are saved, and this holds them.
    Saved,
    /// Nobody has written them yet. Call `write_insights` with this `document_id`, which is paid.
    NotWrittenYet,
    /// A part of a book that is not a chapter, such as its index, gets none.
    NoneForBookPart,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct InsightView {
    /// A short Markdown paragraph.
    pub(crate) text: String,
    /// The places of the pages it rests on, from 1. Give one to `read_page`.
    pub(crate) pages: Vec<u32>,
}

/// What the converted folder of a document holds of insights.
pub(crate) enum Saved {
    Written(ChapterInsights),
    NotWrittenYet,
    NoneForBookPart,
}

impl InsightsView {
    pub(crate) fn truncation(&self) -> &Truncation {
        &self.more
    }

    /// The notes that fit the budget. The units, in order, are each insight, each takeaway and
    /// the cheat sheet.
    fn written(
        document: DocumentHeader,
        file: &Path,
        saved: ChapterInsights,
        budget: &Budget,
    ) -> Result<InsightsView, LibraryError> {
        let ChapterInsights {
            model,
            prompt_version,
            insights,
            takeaways,
            cheat_sheet,
        } = saved;
        let units: Vec<&str> = insights
            .iter()
            .map(|insight| insight.text.as_str())
            .chain(takeaways.iter().map(String::as_str))
            .chain([cheat_sheet.as_str()])
            .collect();
        let taken = budget.take(&units)?;
        let mut view = InsightsView {
            file: Some(file.display().to_string()),
            model: Some(model),
            prompt_version: Some(prompt_version),
            more: taken.truncation,
            ..InsightsView::without(document, SavedState::Saved)
        };
        for slice in taken.slices {
            if let Some(insight) = insights.get(slice.unit) {
                view.insights.push(InsightView {
                    text: slice.text,
                    pages: insight.pages.clone(),
                });
            } else if slice.unit < insights.len() + takeaways.len() {
                view.takeaways.push(slice.text);
            } else {
                view.cheat_sheet = Some(slice.text);
            }
        }
        Ok(view)
    }

    fn without(document: DocumentHeader, state: SavedState) -> InsightsView {
        InsightsView {
            document,
            state,
            file: None,
            model: None,
            prompt_version: None,
            insights: Vec::new(),
            takeaways: Vec::new(),
            cheat_sheet: None,
            more: Truncation::default(),
        }
    }
}

/// The insights saved for the chapter that the document id names. It reads the file in the chapter
/// folder, and the graph only for a document with a title of its own that has no file.
///
/// # Errors
/// A bad argument, a document that no converted chapter has, a file that cannot be read, or a
/// graph that cannot say whether the document is a part of a book.
pub(crate) async fn chapter_insights(
    config: &Config,
    args: ChapterInsightsArgs,
) -> Result<InsightsView, InsightsToolError> {
    let id = parse_document_id(&args.document_id)?;
    let scope = Scope::of("chapter_insights").with(id);
    let budget =
        Budget::new(scope, args.max_chars, args.cursor.as_deref()).map_err(LibraryError::from)?;
    let entry = find_chapter(&config.content_folder, id)?;
    let document = DocumentHeader::of(&entry);
    Ok(match saved_insights(config, id, &entry).await? {
        Saved::Written(saved) => {
            let file = entry.folder.join(INSIGHTS_FILE);
            InsightsView::written(document, &file, saved, &budget)?
        }
        Saved::NotWrittenYet => InsightsView::without(document, SavedState::NotWrittenYet),
        Saved::NoneForBookPart => InsightsView::without(document, SavedState::NoneForBookPart),
    })
}

pub(crate) async fn saved_insights(
    config: &Config,
    id: DocId,
    entry: &ChapterEntry,
) -> Result<Saved, InsightsToolError> {
    if let Some(saved) =
        ChapterInsights::read(&entry.folder).map_err(InsightsToolError::Unreadable)?
    {
        return Ok(Saved::Written(saved));
    }
    let category = category_of(config, id, &entry.index.name).await?;
    Ok(if gets_insights(&entry.index.name, category) {
        Saved::NotWrittenYet
    } else {
        Saved::NoneForBookPart
    })
}

/// A chapter of a book gets insights whatever its media, so only a document with a title of its
/// own needs its category, which the graph holds.
async fn category_of(
    config: &Config,
    id: DocId,
    name: &DocumentName,
) -> Result<Option<Category>, InsightsToolError> {
    match name {
        DocumentName::Chapter { .. } => Ok(None),
        DocumentName::Title(_) => {
            let graph = FalkorGraph::connect(config)
                .await
                .map_err(InsightsToolError::Category)?;
            let document = graph
                .document(id)
                .await
                .map_err(InsightsToolError::Category)?;
            Ok(document.and_then(|document| document.labels.category))
        }
    }
}
