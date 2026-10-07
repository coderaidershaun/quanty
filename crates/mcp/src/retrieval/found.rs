//! What `search` gives back: the items found, each with the reason it is there, and the trace
//! of the steps that found them.

use graph::ConceptNode;
use rag_core::{ConceptHit, ItemPayload};
use rag_retrieval::{Reason, SearchHit, SearchTrace};
use schemars::JsonSchema;
use serde::Serialize;

/// The items a search found, best first.
#[derive(Serialize, JsonSchema)]
pub(crate) struct SearchResult {
    /// The items found, best first. The items that a result cites come last.
    results: Vec<Found>,
    /// How the search got to the results. Only with `explain`.
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<Trace>,
}

/// One stored item that a search found.
#[derive(Serialize, JsonSchema)]
struct Found {
    /// The place of the item in the results, from 1. Other results and answer sources name it.
    number: usize,
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
    /// The title of the document: the book and the chapter.
    document: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    book: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    tags: Vec<String>,
    /// The place of the page in the chapter, from 1. Give it to `read_page`.
    page: u32,
    /// The page number as printed in the book, such as "233" or "xii". Cite a page with it. Left
    /// out when the page shows no number.
    #[serde(skip_serializing_if = "Option::is_none")]
    printed_page: Option<String>,
    /// `chunk`, `formula`, `figure` or `table`.
    kind: String,
    /// The label as printed, such as "(7.3)" or "Figure 13-4". Formulas, figures and tables only.
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// The item as a reader would quote it: the LaTeX of a formula, the Markdown of a table, the
    /// explanation of a figure.
    text: String,
    /// The path of the picture of a figure, on the disk of the machine the server runs on.
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

#[derive(Serialize, JsonSchema)]
struct ConceptView {
    name: String,
    /// What the concept is, in one line.
    definition: String,
}

impl SearchResult {
    /// The hits in order, numbered from 1, and the trace when one was asked for.
    pub(super) fn of(hits: &[SearchHit], trace: Option<SearchTrace>) -> SearchResult {
        SearchResult {
            results: hits
                .iter()
                .enumerate()
                .map(|(index, hit)| Found::of(index + 1, hit))
                .collect(),
            trace: trace.map(Trace::from),
        }
    }
}

impl Found {
    fn of(number: usize, hit: &SearchHit) -> Found {
        Found {
            number,
            item: (&hit.item.payload).into(),
            score: hit.item.score,
            reason: (&hit.reason).into(),
        }
    }
}

impl From<&ItemPayload> for ItemView {
    fn from(payload: &ItemPayload) -> ItemView {
        ItemView {
            document_id: payload.doc_id.to_string(),
            document: payload.doc_title.clone(),
            book: payload.document_labels.book.clone(),
            author: payload.document_labels.author.clone(),
            tags: payload
                .document_labels
                .tags
                .iter()
                .map(ToString::to_string)
                .collect(),
            page: payload.page,
            printed_page: payload.printed_page.clone(),
            kind: payload.kind.as_str().to_owned(),
            label: payload.label.clone(),
            text: payload.text.clone(),
            picture: payload
                .image_path
                .as_ref()
                .map(|path| path.display().to_string()),
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

impl From<&ConceptNode> for ConceptView {
    fn from(concept: &ConceptNode) -> ConceptView {
        ConceptView {
            name: concept.name.clone(),
            definition: concept.definition.clone(),
        }
    }
}
