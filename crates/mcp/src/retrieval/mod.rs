//! The `search` and `answer` tools: reads a question, asks the retriever, and turns what it found
//! into the JSON an agent reads.

mod found;
mod written;

use std::collections::{HashMap, HashSet};

use graph::{FalkorGraph, GraphError};
use rag_core::{
    Category, ConceptStore, Config, DocId, EmbedError, EmptyTag, ItemId, ItemKind, ItemStore,
    LabelFilter, Llm, StoreError, UnknownCategory, UnknownItemKind, WantedDocument, WorkKind,
};
use rag_retrieval::{AnswerError, Retriever, SearchError, SearchHit};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::args::{Budget, BudgetError, Scope, non_blank, parse_optional, parse_tags};
use crate::library::{self, LibraryError, SectionRef};
use crate::services::Services;
use found::Page;
pub(crate) use found::SearchResult;
pub(crate) use written::AnswerResult;

/// What an agent gives `search` and `answer` to say what it wants to know.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct QuestionArgs {
    /// In plain words.
    question: String,
    /// `chunk`, `formula`, `figure`, `table`, `code`.
    kind: Option<String>,
    /// Title or `document_id`.
    document: Option<String>,
    // An agent that still sends the older name `book` keeps its filter. The schema shows `media`
    // only.
    /// Media title.
    #[serde(alias = "book")]
    media: Option<String>,
    /// An author.
    author: Option<String>,
    /// `book`, `paper` or `other`.
    category: Option<String>,
    /// Every tag must match.
    tags: Option<Vec<String>>,
}

/// What an agent gives `search`: the question, and how much to bring back.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct SearchArgs {
    #[serde(flatten)]
    question: QuestionArgs,
    /// Most results.
    limit: Option<u32>,
    /// Add a `trace`.
    explain: Option<bool>,
    /// `full` or `ids`.
    detail: Option<SearchDetail>,
    /// Results to skip.
    offset: Option<u32>,
    /// From a result that has one.
    cursor: Option<String>,
    /// Section `index`; `document` must be an id.
    section: Option<u32>,
    /// Text limit. Default 24000.
    max_chars: Option<u32>,
}

/// How much of each result `search` gives.
#[derive(Deserialize, JsonSchema, Clone, Copy, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchDetail {
    #[default]
    Full,
    Ids,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum RetrievalError {
    #[error("the question is blank; give the question to ask in plain words")]
    BlankQuestion,

    #[error("`kind` is not valid")]
    Kind(#[source] UnknownItemKind),

    #[error("`category` is not valid")]
    Category(#[source] UnknownCategory),

    #[error("a tag in `tags` is blank; give each tag as a word, or leave `tags` out")]
    Tag(#[source] EmptyTag),

    #[error("`limit` is 0; give a whole number from 1, or leave it out")]
    ZeroLimit,

    #[error("give `offset` or `cursor`, not both; `cursor` goes on from the page it came with")]
    OffsetAndCursor,

    #[error(
        "`section` needs `document` to be a `document_id`, not a title; take it from `list_documents`"
    )]
    SectionNeedsDocument,

    #[error(transparent)]
    Budget(#[from] BudgetError),

    #[error(transparent)]
    Section(#[from] LibraryError),

    #[error("could not connect to the graph")]
    Graph(#[from] GraphError),

    #[error("could not set up the embedder")]
    Embedder(#[from] EmbedError),

    #[error("could not set up the item store")]
    ItemStore(#[source] StoreError),

    #[error("could not set up the concept store")]
    ConceptStore(#[source] StoreError),

    #[error("could not search for the question")]
    Search(#[from] SearchError),

    #[error(
        "could not write an answer; call `search` to get the items and write the answer from them"
    )]
    Answer(#[from] AnswerError),
}

/// What the question asks for, once every part of it is read.
struct Asked {
    question: String,
    kind: Option<ItemKind>,
    wanted: LabelFilter,
}

impl QuestionArgs {
    fn read(self) -> Result<Asked, RetrievalError> {
        let question = non_blank(Some(self.question)).ok_or(RetrievalError::BlankQuestion)?;
        let kind = parse_optional::<ItemKind>(self.kind).map_err(RetrievalError::Kind)?;
        let category =
            parse_optional::<Category>(self.category).map_err(RetrievalError::Category)?;
        let tags = parse_tags(self.tags).map_err(RetrievalError::Tag)?;
        Ok(Asked {
            question,
            kind,
            wanted: LabelFilter {
                document: non_blank(self.document)
                    .as_deref()
                    .map(WantedDocument::from),
                media: non_blank(self.media),
                author: non_blank(self.author),
                category,
                tags,
            },
        })
    }
}

/// Finds the items for the question.
///
/// # Errors
/// - a bad argument: a blank question, an unknown kind or category, a blank tag, a `limit` of 0,
///   `offset` with `cursor`, a `section` without a `document_id`, a `max_chars` that is refused or
///   a cursor of another search
/// - a store or the embedder that is not ready, or a search that fails
pub(crate) async fn search<S: Services>(
    config: &Config,
    services: &S,
    args: SearchArgs,
) -> Result<SearchResult, RetrievalError> {
    let asked = args.question.read()?;
    if args.limit == Some(0) {
        return Err(RetrievalError::ZeroLimit);
    }
    let cursor = non_blank(args.cursor);
    if args.offset.is_some() && cursor.is_some() {
        return Err(RetrievalError::OffsetAndCursor);
    }
    let detail = args.detail.unwrap_or_default();
    let items = section_scope(config, &asked, args.section)?;
    let scope = Scope::of("search").with_fingerprint(&format!(
        "{}|{:?}|{:?}|{:?}",
        asked.question, asked.kind, asked.wanted, args.section
    ));
    let budget = Budget::new(scope.clone(), args.max_chars, None)?;
    let first = match cursor {
        Some(cursor) => scope.resume(&cursor)?.unit,
        None => args.offset.unwrap_or(0) as usize,
    };
    let retriever = retriever(config, services).await?;
    let found = retriever
        .search_traced_within(&asked.question, asked.kind, &asked.wanted, items.as_deref())
        .await?;
    let all = found.results.hits;
    let rest = all.get(first..).unwrap_or_default();
    let shown = args
        .limit
        .map_or(rest.len(), |limit| rest.len().min(limit as usize));
    let window = &rest[..shown];
    let sections = match detail {
        SearchDetail::Full => sections_of_hits(config, window),
        SearchDetail::Ids => HashMap::new(),
    };
    let page = Page {
        hits: window,
        first,
        has_more_after: first + window.len() < all.len(),
        detail,
        budget,
        scope,
        sections,
    };
    Ok(SearchResult::of(
        &page,
        args.explain.unwrap_or(false).then_some(found.trace),
        &found.results.usage,
        &config.content_folder,
    )?)
}

/// The stored items of the section that the search looks inside, or `None` to look at every item.
fn section_scope(
    config: &Config,
    asked: &Asked,
    section: Option<u32>,
) -> Result<Option<Vec<ItemId>>, RetrievalError> {
    let Some(index) = section else {
        return Ok(None);
    };
    let Some(WantedDocument::Id(document)) = &asked.wanted.document else {
        return Err(RetrievalError::SectionNeedsDocument);
    };
    let items = library::items_of_section(config, *document, index as usize)?;
    Ok(Some(items))
}

/// The section of each hit's item, by document. The chapter of each document is read once.
fn sections_of_hits(
    config: &Config,
    hits: &[SearchHit],
) -> HashMap<DocId, HashMap<ItemId, SectionRef>> {
    hits.iter()
        .map(|hit| hit.item.payload.doc_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|document| (document, library::sections_of_items(config, document)))
        .collect()
}

/// Writes an answer from the items found for the question. The model is not asked when nothing
/// was found. Whether the model can answer at all is checked first, which costs nothing, so a
/// model that is not ready stops the call before the search is billed.
///
/// # Errors
/// - a bad argument: a blank question, an unknown kind or category, or a blank tag
/// - a store or a service that is not ready, a search that fails, or a reply that breaks a rule
pub(crate) async fn answer<S: Services>(
    config: &Config,
    services: &S,
    args: QuestionArgs,
) -> Result<AnswerResult, RetrievalError> {
    let asked = args.read()?;
    let llm = services.llm(WorkKind::Ask);
    llm.check_ready().await.map_err(AnswerError::Llm)?;
    let retriever = retriever(config, services).await?;
    let results = retriever
        .search(&asked.question, asked.kind, &asked.wanted)
        .await?;
    if results.hits.is_empty() {
        return Ok(AnswerResult::unanswered(&results.usage));
    }
    let written = rag_retrieval::answer(&llm, &asked.question, &results).await?;
    Ok(AnswerResult::from_answer(written, &config.content_folder))
}

/// The graph is connected first, so a store that is down stops the call before anything is
/// embedded and billed.
async fn retriever<S: Services>(
    config: &Config,
    services: &S,
) -> Result<Retriever<S::Embedder>, RetrievalError> {
    let graph = FalkorGraph::connect(config).await?;
    Ok(Retriever {
        embedder: services.embedder(config)?,
        items: ItemStore::connect(config).map_err(RetrievalError::ItemStore)?,
        concepts: ConceptStore::connect(config).map_err(RetrievalError::ConceptStore)?,
        graph,
    })
}
