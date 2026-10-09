//! Answers a question from the stored items: searches, reads the graph and writes the answer.

mod answer;
mod concept_graph;
mod results;
mod trace;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use graph::GraphStore;
use rag_core::{DocId, LabelFilter};
use rag_retrieval::{SearchError, SearchResults, TracedSearch};

use self::results::PieceFacts;
use crate::backend::Reply;
use crate::backend::live::chapters::chapters_on_disk;
use crate::backend::live::{Kept, LiveContext, Services};
use crate::contract::{AskDraft, AskMode, Event, RequestId, SearchReply};

/// A search that finds nothing or fails is the only event: the window settles
/// the later steps itself, and the model is never asked without a result.
pub async fn ask<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    ask: &AskDraft,
    reply: &Reply,
) {
    let Some((retriever, traced)) = search(cx, request, ask, reply).await else {
        return;
    };
    let TracedSearch { results, trace } = traced;
    if results.hits.is_empty() {
        reply.send(Event::Search {
            request,
            result: Ok(SearchReply {
                results: Vec::new(),
                trace: trace::view(&trace, &[]),
                usage: (&results.usage).into(),
            }),
        });
        return;
    }

    let results = Arc::new(results);
    let facts = pieces_on_disk(
        &retriever.graph,
        &results,
        cx.config().content_folder.clone(),
    )
    .await;
    let items = results::result_items(&results.hits, facts);
    let candidates = concept_graph::Candidates::new(&trace, &items);
    reply.send(Event::Search {
        request,
        result: Ok(SearchReply {
            results: items,
            trace: trace::view(&trace, &results.hits),
            usage: (&results.usage).into(),
        }),
    });

    let concept_graph = concept_graph::read(&retriever.graph, candidates)
        .await
        .map_err(|error| cx.failure(error));
    reply.send(Event::Graph {
        request,
        result: concept_graph,
    });
    if ask.mode == AskMode::ResultsOnly {
        return;
    }

    let answer = rag_retrieval::answer(&cx.answer_llm(), &ask.question, &results)
        .await
        .map(answer::view)
        .map_err(|error| cx.failure(error));
    reply.send(Event::Answer {
        request,
        result: answer,
    });
}

/// `None` when the search has already sent the event that ends the ask: its failure, or the empty
/// reply of the first run of the app.
async fn search<S: Services>(
    cx: &LiveContext<S>,
    request: RequestId,
    ask: &AskDraft,
    reply: &Reply,
) -> Option<(Kept<S>, TracedSearch)> {
    let wanted = LabelFilter::from(&ask.filters);
    let retriever = match cx.retriever().await {
        Ok(retriever) => retriever,
        Err(failure) => {
            reply.send(Event::Search {
                request,
                result: Err(failure),
            });
            return None;
        }
    };
    let error = match retriever.search_traced(&ask.question, None, &wanted).await {
        Ok(traced) => return Some((retriever, traced)),
        Err(error) => error,
    };
    // Stores with nothing in them cannot be searched yet, and that is the first run of
    // the app, not a store that is down.
    let is_first_run = matches!(error, SearchError::Items(_))
        && matches!(retriever.items.collection_exists().await, Ok(false));
    let result = if is_first_run {
        Ok(SearchReply::default())
    } else {
        Err(cx.failure(error))
    };
    reply.send(Event::Search { request, result });
    None
}

/// A reading that does not finish gives an empty list, which leaves every hit without facts.
/// Neither that nor folders that cannot be read stop the ask, because the facts are optional.
async fn pieces_on_disk<G: GraphStore>(
    graph: &G,
    results: &Arc<SearchResults>,
    content_folder: PathBuf,
) -> Vec<Option<PieceFacts>> {
    let mut seen = HashSet::new();
    let documents: Vec<DocId> = results
        .hits
        .iter()
        .map(|hit| hit.item.payload.doc_id)
        .filter(|id| seen.insert(*id))
        .collect();
    let stored_folders = graph
        .chapter_folders(&documents)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(?error, "could not read the folders of the documents");
            HashMap::new()
        });
    let results = Arc::clone(results);
    tokio::task::spawn_blocking(move || {
        let documents = documents
            .into_iter()
            .map(|id| (id, stored_folders.get(&id).map(PathBuf::as_path)));
        let folders: HashMap<DocId, PathBuf> = chapters_on_disk(documents, &content_folder)
            .into_iter()
            .map(|(id, entry)| (id, entry.folder))
            .collect();
        results::piece_facts(&results.hits, &folders)
    })
    .await
    .unwrap_or_else(|error| {
        tracing::warn!(?error, "the reading of the chapters did not finish");
        Vec::new()
    })
}
