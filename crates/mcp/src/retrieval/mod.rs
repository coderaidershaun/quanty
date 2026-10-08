//! The `search` and `answer` tools: reads a question, asks the retriever, and turns what it found
//! into the JSON an agent reads.

mod found;
mod written;

use std::collections::BTreeSet;

use graph::{FalkorGraph, GraphError};
use rag_core::{
    ConceptStore, Config, DocumentLabels, EmbedError, EmptyTag, ItemKind, ItemStore, Llm,
    StoreError, Tag, UnknownItemKind,
};
use rag_retrieval::{ANSWER_MODEL, AnswerError, Retriever, SearchError};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::services::Services;
pub(crate) use found::SearchResult;
pub(crate) use written::AnswerResult;

/// What an agent gives `search` and `answer` to say what it wants to know.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct QuestionArgs {
    /// The question, in plain words, as a person would ask it.
    question: String,
    /// Look only at items of this kind: `chunk` (running text), `formula`, `figure` or `table`.
    kind: Option<String>,
    /// Look only at documents of this book, whatever its capitals.
    book: Option<String>,
    /// Look only at documents by this author, whatever its capitals.
    author: Option<String>,
    /// Look only at documents that have every one of these tags.
    tags: Option<Vec<String>>,
}

/// What an agent gives `search`: the question, and how much to bring back.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct SearchArgs {
    #[serde(flatten)]
    question: QuestionArgs,
    /// Keep only the first `limit` results. A whole number from 1.
    limit: Option<u32>,
    /// Set to true to add `trace`, which says how each step of the search got to the results.
    explain: Option<bool>,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum RetrievalError {
    #[error("the question is blank; give the question to ask in plain words")]
    BlankQuestion,

    #[error("`kind` is not valid")]
    Kind(#[source] UnknownItemKind),

    #[error("a tag in `tags` is blank; give each tag as a word, or leave `tags` out")]
    Tag(#[source] EmptyTag),

    #[error("`limit` is 0; give a whole number from 1, or leave it out")]
    ZeroLimit,

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
    wanted: DocumentLabels,
}

impl QuestionArgs {
    fn read(self) -> Result<Asked, RetrievalError> {
        let question = non_blank(Some(self.question)).ok_or(RetrievalError::BlankQuestion)?;
        let kind = non_blank(self.kind)
            .map(|kind| kind.parse::<ItemKind>())
            .transpose()
            .map_err(RetrievalError::Kind)?;
        let tags = self
            .tags
            .unwrap_or_default()
            .iter()
            .map(|tag| tag.parse::<Tag>())
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(RetrievalError::Tag)?;
        Ok(Asked {
            question,
            kind,
            wanted: DocumentLabels {
                book: non_blank(self.book),
                author: non_blank(self.author),
                tags,
            },
        })
    }
}

/// The text with no space at either end, or `None` when nothing is left.
fn non_blank(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Finds the items for the question.
///
/// # Errors
/// - a bad argument: a blank question, an unknown kind, a blank tag or a `limit` of 0
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
    let retriever = retriever(config, services).await?;
    let found = retriever
        .search_traced(&asked.question, asked.kind, &asked.wanted)
        .await?;
    let mut hits = found.results.hits;
    if let Some(limit) = args.limit {
        hits.truncate(limit as usize);
    }
    Ok(SearchResult::of(
        &hits,
        args.explain.unwrap_or(false).then_some(found.trace),
    ))
}

/// Writes an answer from the items found for the question. The model is not asked when nothing
/// was found. Whether the model can answer at all is checked first, which costs nothing, so a
/// model that is not ready stops the call before the search is billed.
///
/// # Errors
/// - a bad argument: a blank question, an unknown kind or a blank tag
/// - a store or a service that is not ready, a search that fails, or a reply that breaks a rule
pub(crate) async fn answer<S: Services>(
    config: &Config,
    services: &S,
    args: QuestionArgs,
) -> Result<AnswerResult, RetrievalError> {
    let asked = args.read()?;
    let llm = services.llm(ANSWER_MODEL);
    llm.check_ready().await.map_err(AnswerError::Llm)?;
    let retriever = retriever(config, services).await?;
    let results = retriever
        .search(&asked.question, asked.kind, &asked.wanted)
        .await?;
    if results.hits.is_empty() {
        return Ok(AnswerResult::unanswered());
    }
    let written = rag_retrieval::answer(&llm, &asked.question, &results).await?;
    Ok(written.into())
}

/// The graph is connected first, so a store that is down stops the call before anything is
/// embedded and billed.
async fn retriever<S: Services>(
    config: &Config,
    services: &S,
) -> Result<Retriever<S::Embedder, FalkorGraph>, RetrievalError> {
    let graph = FalkorGraph::connect(config).await?;
    Ok(Retriever {
        embedder: services.embedder(config)?,
        items: ItemStore::connect(config).map_err(RetrievalError::ItemStore)?,
        concepts: ConceptStore::connect(config).map_err(RetrievalError::ConceptStore)?,
        graph,
    })
}
