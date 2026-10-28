//! The `document_overview` tool: one call for what a document holds and says: its insights, its
//! takeaways and cheat sheet, its concepts, its outline, and the documents that share its concepts.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::Path;

use graph::{FalkorGraph, GraphError};
use rag_core::{ConceptId, Config, DocId, ItemId, error_chain};
use rag_ingestion::{assess_artifacts, document_title};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::mentions::DocumentConcepts;
use super::outline::{SectionRow, section_rows};
use crate::library::chapter::{DocumentHeader, Whole, parse_document_id};
use crate::library::contents::ContentsCounts;
// SMELL: the saved insights are read through the insights module, which reads the library module
// for the header of a document, so the two modules depend on each other.
use crate::insights::{InsightView, InsightsToolError, Saved, SavedState, saved_insights};

/// How many concepts a summary names.
const SUMMARY_CONCEPTS: usize = 10;

/// How many concepts the full overview names.
const FULL_CONCEPTS: usize = 50;

/// Documents that share fewer concepts than this are not related.
const FEWEST_SHARED_TO_RELATE: usize = 3;

/// How many related documents the full overview names.
const RELATED_DOCUMENTS: usize = 5;

/// What an agent gives `document_overview`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct DocumentOverviewArgs {
    /// From `list_documents`.
    document_id: String,
    /// `summary` or `full`.
    detail: Option<OverviewDetail>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OverviewDetail {
    /// The header, the item counts, the takeaways and the top ten concepts.
    #[default]
    Summary,
    /// Also the outline, every insight, the cheat sheet, fifty concepts with definitions, the
    /// related documents and the artifacts.
    Full,
}

impl OverviewDetail {
    fn is_full(self) -> bool {
        self == OverviewDetail::Full
    }

    /// How many concepts the overview names.
    fn concepts(self) -> usize {
        match self {
            OverviewDetail::Summary => SUMMARY_CONCEPTS,
            OverviewDetail::Full => FULL_CONCEPTS,
        }
    }
}

/// What a document holds and says.
#[derive(Serialize, JsonSchema)]
pub(crate) struct Overview {
    #[serde(flatten)]
    document: DocumentHeader,
    /// The title that `list_documents` gives.
    title: String,
    /// The authors of the media. Left out when none is known or the graph is down.
    #[serde(skip_serializing_if = "Option::is_none")]
    authors: Option<Vec<String>>,
    /// How many items of each kind the chapter has.
    items: ContentsCounts,
    insights: Fallible<OverviewInsights>,
    /// The concepts its items mention, the most mentioned first.
    concepts: Fallible<Vec<OverviewConcept>>,
    /// False when an ingest stopped or skipped an item. Left out when the graph is down.
    #[serde(skip_serializing_if = "Option::is_none")]
    ingested_whole: Option<bool>,
    /// The sections, as `document_outline` gives them. Only with `detail: full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    outline: Option<Vec<SectionRow>>,
    /// Documents that share three concepts or more with this one. Only with `detail: full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    related_documents: Option<Fallible<Vec<RelatedDocument>>>,
    /// Only with `detail: full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    artifacts: Option<Artifacts>,
}

/// A part of the overview, or the words of why it could not be read.
#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
enum Fallible<T> {
    Read(T),
    Failed { error: String },
}

#[derive(Serialize, JsonSchema)]
struct OverviewInsights {
    state: SavedState,
    /// Only with `detail: full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    insights: Option<Vec<InsightView>>,
    takeaways: Vec<String>,
    /// Only with `detail: full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    cheat_sheet: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct OverviewConcept {
    concept_id: String,
    name: String,
    /// How many items of this document mention it.
    mentions: usize,
    /// Only with `detail: full`.
    #[serde(skip_serializing_if = "Option::is_none")]
    definition: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct RelatedDocument {
    document_id: String,
    title: String,
    /// How many concepts it shares with this document.
    shared: usize,
}

#[derive(Serialize, JsonSchema)]
struct Artifacts {
    /// `ready`, `missing`, `stale`, `partial`, `too_large` or `no_folder`.
    state: &'static str,
    done: usize,
    total: usize,
}

/// What the graph gave for the overview.
struct FromGraph {
    authors: Vec<String>,
    concepts: DocumentConcepts,
    ingested_whole: bool,
    related: Vec<RelatedDocument>,
}

/// The overview of the document that the id names. The chapter and the saved insights are read
/// from disk first. The graph gives the concepts; when it is down they are an error and the rest
/// is still given.
///
/// # Errors
/// A bad argument, a document that no converted chapter has, a chapter or saved insights that
/// cannot be read.
pub(crate) async fn document_overview(
    config: &Config,
    args: DocumentOverviewArgs,
) -> Result<Overview, InsightsToolError> {
    let detail = args.detail.unwrap_or_default();
    let id = parse_document_id(&args.document_id)?;
    let whole = Whole::find(&config.content_folder, id)?;
    let insights = match saved_insights(config, id, &whole.entry).await {
        Ok(saved) => Fallible::Read(OverviewInsights::of(saved, detail)),
        Err(InsightsToolError::Category(error)) => Fallible::Failed {
            error: error_chain(&error),
        },
        Err(other) => return Err(other),
    };
    let from_graph = from_graph(config, id, &whole, detail).await;
    let rows = detail.is_full().then(|| {
        let concepts = from_graph.as_ref().ok().map(|found| &found.concepts);
        section_rows(&whole, concepts)
    });
    let (authors, concepts, ingested_whole, related) = match from_graph {
        Ok(found) => (
            Some(found.authors).filter(|authors| !authors.is_empty()),
            Fallible::Read(concept_rows(&found.concepts, detail)),
            Some(found.ingested_whole),
            Fallible::Read(found.related),
        ),
        Err(error) => (
            None,
            Fallible::Failed {
                error: error.clone(),
            },
            None,
            Fallible::Failed { error },
        ),
    };
    Ok(Overview {
        document: DocumentHeader::of(&whole.entry),
        title: document_title(&whole.entry.index),
        authors,
        items: ContentsCounts::of(&whole.chapter.items()),
        insights,
        concepts,
        ingested_whole,
        outline: rows,
        related_documents: detail.is_full().then_some(related),
        artifacts: detail.is_full().then(|| Artifacts::of(&whole.entry.folder)),
    })
}

impl Artifacts {
    fn of(folder: &Path) -> Artifacts {
        let assessed = assess_artifacts(folder);
        Artifacts {
            state: assessed.state.as_str(),
            done: assessed.done,
            total: assessed.total,
        }
    }
}

impl OverviewInsights {
    fn of(saved: Saved, detail: OverviewDetail) -> OverviewInsights {
        let (state, insights, takeaways, cheat_sheet) = match saved {
            Saved::Written(saved) => (
                SavedState::Saved,
                saved
                    .insights
                    .into_iter()
                    .map(|insight| InsightView {
                        text: insight.text,
                        pages: insight.pages,
                    })
                    .collect(),
                saved.takeaways,
                Some(saved.cheat_sheet),
            ),
            Saved::NotWrittenYet => (SavedState::NotWrittenYet, Vec::new(), Vec::new(), None),
            Saved::NoneForBookPart => (SavedState::NoneForBookPart, Vec::new(), Vec::new(), None),
        };
        OverviewInsights {
            state,
            insights: detail.is_full().then_some(insights),
            takeaways,
            cheat_sheet: cheat_sheet.filter(|_| detail.is_full()),
        }
    }
}

/// The concepts, the mark that the document is whole and, for the full overview, the related
/// documents, or the words of why the graph could not give them.
async fn from_graph(
    config: &Config,
    id: DocId,
    whole: &Whole,
    detail: OverviewDetail,
) -> Result<FromGraph, String> {
    let item_ids: Vec<ItemId> = whole.items.iter().map(|item| item.id).collect();
    let read = async {
        let graph = FalkorGraph::connect(config).await?;
        let authors = graph
            .document(id)
            .await?
            .map(|node| node.labels.authors)
            .unwrap_or_default();
        let concepts = DocumentConcepts::read(&graph, id, &item_ids).await?;
        let ingested_whole = graph.ingested_items(id).await?.is_some();
        let related = if detail.is_full() {
            related_documents(&graph, id, &concepts).await?
        } else {
            Vec::new()
        };
        Ok::<_, GraphError>(FromGraph {
            authors,
            concepts,
            ingested_whole,
            related,
        })
    };
    read.await.map_err(|error| error_chain(&error))
}

/// The other documents that share enough concepts with this one, the most shared first.
// SMELL: this reads the graph once for each stored document, so a full overview gets slower as
// the library grows. One graph query that counts the shared concepts would fix it.
async fn related_documents(
    graph: &FalkorGraph,
    id: DocId,
    concepts: &DocumentConcepts,
) -> Result<Vec<RelatedDocument>, GraphError> {
    let own: HashSet<ConceptId> = concepts.concepts.iter().map(|concept| concept.id).collect();
    let mut related = Vec::new();
    for record in graph.document_records().await? {
        if record.node.id == id {
            continue;
        }
        let shared = graph
            .concepts_in_document(record.node.id)
            .await?
            .iter()
            .filter(|concept| own.contains(&concept.id))
            .count();
        if shared >= FEWEST_SHARED_TO_RELATE {
            related.push(RelatedDocument {
                document_id: record.node.id.to_string(),
                title: record.node.title,
                shared,
            });
        }
    }
    related.sort_by(|a, b| (Reverse(a.shared), &a.title).cmp(&(Reverse(b.shared), &b.title)));
    related.truncate(RELATED_DOCUMENTS);
    Ok(related)
}

/// The concepts with how many items of the document mention each, the most mentioned first.
fn concept_rows(concepts: &DocumentConcepts, detail: OverviewDetail) -> Vec<OverviewConcept> {
    let mut rows: Vec<OverviewConcept> = concepts
        .concepts
        .iter()
        .map(|concept| OverviewConcept {
            concept_id: concept.id.to_string(),
            name: concept.name.clone(),
            mentions: concepts.mentions_of(concept.id),
            definition: detail.is_full().then(|| concept.definition.clone()),
        })
        .collect();
    rows.sort_by(|a, b| (Reverse(a.mentions), &a.name).cmp(&(Reverse(b.mentions), &b.name)));
    rows.truncate(detail.concepts());
    rows
}
