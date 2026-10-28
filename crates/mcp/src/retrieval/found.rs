//! What `search` gives back: the items found, each with the reason it is there, and the trace
//! of the steps that found them.

use std::collections::HashMap;
use std::path::Path;

use rag_core::{ConceptHit, DocId, ItemId, ItemPayload, UsageTally};
use rag_retrieval::{Reason, SearchHit, SearchTrace};
use schemars::JsonSchema;
use serde::Serialize;

use super::SearchDetail;
use crate::args::{Budget, BudgetError, Position, Scope, Truncation};
use crate::library::{ConceptView, SectionRef};
use crate::usage::Spent;

/// The part of the results that one call shows, and what it needs to say where the next call goes
/// on.
pub(super) struct Page<'a> {
    pub(super) hits: &'a [SearchHit],
    /// The place, counted from 0, of the first of the hits among all the results.
    pub(super) first: usize,
    /// Whether results come after the hits, which `limit` left out.
    pub(super) has_more_after: bool,
    pub(super) detail: SearchDetail,
    pub(super) budget: Budget,
    pub(super) scope: Scope,
    /// The section of each item, by document. Empty with `detail: ids`.
    pub(super) sections: HashMap<DocId, HashMap<ItemId, SectionRef>>,
}

impl Page<'_> {
    /// The rows with their texts, as many as the budget holds. Only the first row is ever cut, so
    /// that each call shows a result.
    fn full_rows(&self, content: &Path) -> Result<(Vec<Found>, Truncation), BudgetError> {
        let texts: Vec<&str> = self
            .hits
            .iter()
            .map(|hit| hit.item.payload.text.as_str())
            .collect();
        let taken = self
            .budget
            .take_keeping_heads(&texts, |unit| texts[unit].chars().count())?;
        let rows = taken
            .slices
            .into_iter()
            .map(|slice| {
                let hit = &self.hits[slice.unit];
                let section = self
                    .sections
                    .get(&hit.item.payload.doc_id)
                    .and_then(|places| places.get(&hit.item.id))
                    .cloned();
                let number = self.first + slice.unit + 1;
                Found::of(number, hit, content, section, slice.text)
            })
            .collect();
        Ok((rows, taken.truncation))
    }
}

/// The items a search found, best first.
#[derive(Serialize, JsonSchema)]
pub(crate) struct SearchResult {
    /// The items found, best first. The items that a result cites come last.
    results: Rows,
    /// How the search got to the results. Only with `explain`.
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<Trace>,
    /// `truncated` is true when a text was cut or a result left out for `max_chars`. `cursor` is
    /// there whenever results are left for the next call.
    #[serde(flatten)]
    more: Truncation,
    /// What embedding the question used, estimated.
    #[serde(flatten)]
    spent: Spent,
}

/// The results, whole or as ids only.
#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
enum Rows {
    Full(Vec<Found>),
    Ids(Vec<FoundId>),
}

/// One result of a search with `detail: ids`.
#[derive(Serialize, JsonSchema)]
struct FoundId {
    number: usize,
    item_id: String,
    document_id: String,
    page: u32,
    kind: String,
    score: f32,
}

/// One stored item that a search found.
#[derive(Serialize, JsonSchema)]
struct Found {
    /// The place of the item in the results, from 1. Other results and answer sources name it.
    number: usize,
    /// Give it to `read_item`.
    item_id: String,
    /// The section of the document that holds the item. Left out when the document is not
    /// converted on this machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    section: Option<SectionRef>,
    #[serde(flatten)]
    item: ItemView,
    /// How near the item is to the question, from 0 to 1. Higher is nearer.
    score: f32,
    /// Why the item is in the results.
    reason: Why,
}

/// What every stored item says about itself, in a search result and in an answer source.
#[derive(Serialize, JsonSchema)]
pub(super) struct ItemView {
    /// The document the item is from. Give it to `read_page`.
    document_id: String,
    /// The title of the document: the media and the chapter, or a title of its own.
    document: String,
    /// The title of the media the document belongs to. A picture that stands alone has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    media: Option<String>,
    /// `book`, `paper` or `other`: the category of the media.
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    /// The authors of the media.
    authors: Vec<String>,
    /// The tags of the media.
    media_tags: Vec<String>,
    /// The tags of the document itself.
    tags: Vec<String>,
    /// The place of the page in the chapter, from 1. Give it to `read_page`.
    page: u32,
    /// The page number as printed in the book, such as "233" or "xii". Cite a page with it. Left
    /// out when the page shows no number.
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    /// The place of the page where a code listing that page breaks cut ends. Left out for every
    /// other item.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_page: Option<u32>,
    /// The number printed on that page.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_printed_page: Option<String>,
    /// `chunk`, `formula`, `figure`, `table` or `code`.
    kind: String,
    /// The label as printed, such as "(7.3)" or "Figure 13-4". Formulas, figures, tables and code
    /// only.
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// The language of a code listing in lower case, such as "python". Left out when the language
    /// is not known.
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    /// The item as a reader would quote it: the LaTeX of a formula, the Markdown of a table, the
    /// explanation of a figure, the code as printed for code.
    text: String,
    /// The picture URL relative to the app base, contained beneath content/.
    #[serde(skip_serializing_if = "Option::is_none")]
    picture: Option<String>,
}

/// Why an item is in the results.
#[derive(Serialize, JsonSchema)]
#[serde(tag = "why", rename_all = "snake_case")]
enum Why {
    /// It is one of the items nearest to the question.
    Nearest,
    /// The graph led to it through a concept of this name.
    Concept { concept: String },
    /// The result `by` cites it by its printed label.
    Cited { by: usize, label: String },
}

/// What each step of a search produced.
#[derive(Serialize, JsonSchema)]
struct Trace {
    /// How many documents carry every wanted label. Left out when no label was wanted.
    #[serde(skip_serializing_if = "Option::is_none")]
    documents_searched: Option<usize>,
    /// How many items were nearest to the question, and so the start of the search.
    seeds: usize,
    /// The concepts nearest to the question.
    question_concepts: Vec<ConceptScore>,
    /// The concepts that the nearest items mention.
    seed_concepts: Vec<ConceptView>,
    /// The concepts one relation away from the concepts above.
    related_concepts: Vec<ConceptView>,
    /// How many items went on to be ranked.
    candidates: usize,
    /// How many of them the item store gave back.
    ranked: usize,
    /// How many ranked items were passed over because their document already gave enough results.
    capped: usize,
    /// How many results were kept before the items that they cite were added.
    kept: usize,
}

#[derive(Serialize, JsonSchema)]
struct ConceptScore {
    name: String,
    score: f32,
}

impl SearchResult {
    /// The hits in order, numbered from 1 among all the results, the trace when one was asked
    /// for, and what the search used.
    ///
    /// # Errors
    /// A budget that cannot be taken from.
    pub(super) fn of(
        page: &Page<'_>,
        trace: Option<SearchTrace>,
        usage: &UsageTally,
        content: &Path,
    ) -> Result<SearchResult, BudgetError> {
        let (results, shown, truncation) = match page.detail {
            SearchDetail::Full => {
                let (rows, truncation) = page.full_rows(content)?;
                let shown = rows.len();
                (Rows::Full(rows), shown, truncation)
            }
            SearchDetail::Ids => {
                let rows = (page.first + 1..)
                    .zip(page.hits)
                    .map(|(number, hit)| FoundId::of(number, hit))
                    .collect();
                (Rows::Ids(rows), page.hits.len(), Truncation::whole(0))
            }
        };
        let has_more = page.has_more_after || shown < page.hits.len();
        let cursor = has_more.then(|| {
            page.scope.cursor(Position {
                unit: page.first + shown,
                offset: 0,
            })
        });
        Ok(SearchResult {
            results,
            trace: trace.map(Trace::from),
            more: truncation.paged_by(cursor),
            spent: usage.into(),
        })
    }
}

impl FoundId {
    fn of(number: usize, hit: &SearchHit) -> FoundId {
        FoundId {
            number,
            item_id: hit.item.id.to_string(),
            document_id: hit.item.payload.doc_id.to_string(),
            page: hit.item.payload.page,
            kind: hit.item.payload.kind.as_str().to_owned(),
            score: hit.item.score,
        }
    }
}

impl Found {
    fn of(
        number: usize,
        hit: &SearchHit,
        content: &Path,
        section: Option<SectionRef>,
        text: String,
    ) -> Found {
        Found {
            number,
            item_id: hit.item.id.to_string(),
            section,
            item: ItemView {
                text,
                ..ItemView::from_payload(&hit.item.payload, content)
            },
            score: hit.item.score,
            reason: (&hit.reason).into(),
        }
    }
}

impl ItemView {
    pub(super) fn from_payload(payload: &ItemPayload, content: &std::path::Path) -> ItemView {
        let labels = &payload.document_labels;
        ItemView {
            document_id: payload.doc_id.to_string(),
            document: payload.doc_title.clone(),
            media: labels.media.clone(),
            category: labels.category.map(|category| category.as_str().to_owned()),
            authors: labels.authors.clone(),
            media_tags: labels.media_tags.iter().map(ToString::to_string).collect(),
            tags: labels.tags.iter().map(ToString::to_string).collect(),
            page: payload.page,
            printed_page: payload.printed_page.clone(),
            last_page: payload.last_page,
            last_printed_page: payload.last_printed_page.clone(),
            kind: payload.kind.as_str().to_owned(),
            label: payload.label.clone(),
            language: payload.language.clone(),
            text: payload.text.clone(),
            picture: payload
                .image_path
                .as_ref()
                .and_then(|path| rag_core::content_url(path, content)),
        }
    }
}

impl From<&Reason> for Why {
    fn from(reason: &Reason) -> Why {
        match reason {
            Reason::Nearest => Why::Nearest,
            Reason::Concept(name) => Why::Concept {
                concept: name.clone(),
            },
            Reason::Cited { by, label } => Why::Cited {
                by: *by,
                label: label.clone(),
            },
        }
    }
}

impl From<SearchTrace> for Trace {
    fn from(trace: SearchTrace) -> Trace {
        Trace {
            documents_searched: trace.documents_searched,
            seeds: trace.seeds.len(),
            question_concepts: trace
                .question_concepts
                .iter()
                .map(ConceptScore::from)
                .collect(),
            seed_concepts: trace.seed_concepts.iter().map(ConceptView::from).collect(),
            related_concepts: trace
                .related_concepts
                .iter()
                .map(ConceptView::from)
                .collect(),
            candidates: trace.candidates,
            ranked: trace.ranked,
            capped: trace.capped.len(),
            kept: trace.kept,
        }
    }
}

impl From<&ConceptHit> for ConceptScore {
    fn from(hit: &ConceptHit) -> ConceptScore {
        ConceptScore {
            name: hit.name.clone(),
            score: hit.score,
        }
    }
}
