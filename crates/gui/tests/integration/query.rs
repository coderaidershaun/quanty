//! Checks that an ask on the live backend finds the stored items and writes the answer.

use gui::backend::Reply;
use gui::backend::live::{LiveContext, query};
use gui::contract::{
    AnswerBlock, AskDraft, AskMode, ConceptId, DocId, EdgeKind, Event, Filters, GraphEdge,
    GraphNode, ItemId, ItemKind, NodeId, NodeKind, Reason, RequestId, ResultItem,
};
use rag_core::Config;
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores};
use rag_retrieval::RESULTS_PER_QUERY;
use serde_json::json;

use crate::support::{
    Folders, IN_DEPTH, INTUITION, QUESTION, StandInServices, context_over, context_with, fill,
};

const REQUEST: RequestId = RequestId(7);
const TABLE_CAPTION: &str = "How the price of a call and of a put respond when one input rises.";
/// The folder of Notes chapter 2 is kept by the graph, and Notes chapter 1 lies under the content
/// folder.
const FOLDERS: Folders = Folders {
    stored: IN_DEPTH,
    copied: INTUITION,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored query::"]
async fn an_ask_sends_the_results_with_their_trace_then_the_graph_then_the_answer() {
    let answering = StandInLlm::replying(|_, _| {
        Ok(json!({
            "title": "How a call is priced",
            "claims": [
                { "heading": "", "text": "A call is priced by the formula.", "sources": [1] },
                { "heading": "Key assumptions", "text": "The price assumes a table.", "sources": [2, 1] },
            ],
            "follow_ups": ["What is a put?"],
        }))
    });
    let world = fill("query-ask", &answering, FOLDERS).await;

    let events = ask(&world.cx).await;

    let [
        Event::Search {
            request: REQUEST,
            result: Ok(search),
        },
        Event::Graph {
            request: REQUEST,
            result: Ok(graph),
        },
        Event::Answer {
            request: REQUEST,
            result: Ok(answer),
        },
    ] = events.as_slice()
    else {
        panic!("expected the search, the graph and the answer, with id 7: {events:#?}");
    };

    let results = &search.results;
    let numbers: Vec<usize> = results.iter().map(|result| result.number).collect();
    assert_eq!(numbers, (1..=results.len()).collect::<Vec<_>>());
    let [formula, table, chunk, figure, ..] = results.as_slice() else {
        panic!("expected at least four results: {results:#?}");
    };
    let picks = &world.picks;
    assert_eq!(formula.id, ItemId::from(picks.formula.id));
    assert_eq!(formula.doc, DocId::from(picks.formula.payload.doc_id));
    assert_eq!(formula.kind, ItemKind::Formula);
    assert_eq!(formula.reason, Reason::Nearest);
    assert_eq!(formula.label.as_deref(), Some("(2.4)"));
    assert_eq!(formula.text, picks.formula.payload.text);
    assert_eq!(formula.printed_page.as_deref(), Some("7"));
    assert_eq!(formula.media.as_deref(), Some("Quanty Sample Notes"));
    assert_eq!(
        formula.doc_title,
        "Quanty Sample Notes, chapter 2: Black Scholes In Depth"
    );
    // The graph keeps the folder of this chapter.
    assert_eq!(
        on_disk(formula),
        (Some(5), Some("Black–Scholes call price"), None)
    );
    // Only the content folder has this chapter, and it is found by the id of the document.
    assert_eq!(on_disk(table), (Some(3), None, Some(TABLE_CAPTION)));
    assert_eq!(chunk.id, ItemId::from(picks.chunk.id));
    assert_eq!(on_disk(chunk), (None, None, None));
    assert_eq!(figure.kind, ItemKind::Figure);
    assert_eq!(figure.label.as_deref(), Some("Figure 13-4"));
    assert_eq!(
        figure.image.as_ref().map(|image| image.path.as_path()),
        picks.figure.payload.image_path.as_deref()
    );
    // Neither the graph nor the content folder has this chapter.
    assert_eq!(on_disk(figure), (None, None, None));

    let trace = &search.trace;
    let cited: Vec<String> = results
        .iter()
        .filter_map(|result| match &result.reason {
            Reason::Cited { label, .. } => Some(label.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(trace.documents_searched, None);
    assert_eq!(trace.nearest, RESULTS_PER_QUERY);
    assert_eq!(
        trace.seed_concepts,
        Some(vec!["Black–Scholes model".to_owned()])
    );
    assert_eq!(trace.related_concepts, Some(vec!["Itô's lemma".to_owned()]));
    assert_eq!(trace.kept, Some(results.len() - cited.len()));
    assert_eq!(trace.cited, Some(cited));
    assert!(trace.candidates >= trace.ranked && trace.ranked >= trace.kept);

    let scholes = NodeId::Concept(ConceptId::from(world.black_scholes.id));
    let lemma = NodeId::Concept(ConceptId::from(world.lemma.id));
    let definition = Some(world.black_scholes.definition.as_str());
    assert_eq!(
        graph.nodes,
        [
            node(
                scholes,
                NodeKind::Concept,
                "Black–Scholes model",
                definition
            ),
            node(lemma, NodeKind::Related, "Itô's lemma", None),
            node(
                NodeId::Item(1),
                NodeKind::Formula,
                "Formula (2.4)",
                Some("Black–Scholes call price")
            ),
        ]
    );
    assert_eq!(
        graph.edges,
        [
            edge(scholes, lemma, EdgeKind::DerivedFrom),
            edge(NodeId::Item(1), scholes, EdgeKind::Mentions),
        ]
    );

    assert_eq!(answer.title.as_deref(), Some("How a call is priced"));
    assert_eq!(
        answer.blocks,
        [
            paragraph("A call is priced by the formula.", &[1]),
            AnswerBlock::Item(1),
            AnswerBlock::Heading("Key assumptions".to_owned()),
            paragraph("The price assumes a table.", &[2, 1]),
            AnswerBlock::Item(2),
        ]
    );
    assert_eq!(answer.follow_ups, ["What is a put?"]);
    let asked = answering.questions();
    assert_eq!(asked.len(), 1);
    assert!(asked[0].input.contains(QUESTION));
    assert!(asked[0].input.contains(&picks.formula.payload.text));
    assert!(asked[0].input.contains(&picks.table.payload.text));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored query::"]
async fn an_ask_with_nothing_stored_sends_an_empty_search_and_asks_no_model() {
    let answering = StandInLlm::finding_nothing();
    let stores = ThrowawayStores::new("query-empty");
    let cx = context_with(stores, &answering, StandInEmbedder::default);

    let events = ask(&cx).await;

    let [
        Event::Search {
            request: REQUEST,
            result: Ok(search),
        },
    ] = events.as_slice()
    else {
        panic!("expected the search alone, with id 7: {events:#?}");
    };
    assert!(search.results.is_empty());
    assert_eq!(search.trace.documents_searched, None);
    assert_eq!(search.trace.seed_concepts, None);
    assert_eq!(answering.calls(), 0);

    // A store that does not answer is a failure, not the empty state.
    let stores = ThrowawayStores::new("query-qdrant-down");
    let config = Config {
        qdrant_url: "http://127.0.0.1:1".to_owned(),
        ..stores.config().clone()
    };
    let cx = context_over(config, stores, &answering, StandInEmbedder::default);

    let events = ask(&cx).await;

    let [
        Event::Search {
            request: REQUEST,
            result: Err(_),
        },
    ] = events.as_slice()
    else {
        panic!("expected a failed search alone, with id 7: {events:#?}");
    };
    assert_eq!(answering.calls(), 0);
}

fn paragraph(text: &str, cites: &[usize]) -> AnswerBlock {
    AnswerBlock::Paragraph {
        text: text.to_owned(),
        cites: cites.to_vec(),
    }
}

fn node(id: NodeId, kind: NodeKind, label: &str, detail: Option<&str>) -> GraphNode {
    GraphNode {
        id,
        kind,
        label: label.to_owned(),
        detail: detail.map(str::to_owned),
    }
}

fn edge(from: NodeId, to: NodeId, kind: EdgeKind) -> GraphEdge {
    GraphEdge { from, to, kind }
}

/// The piece, name and caption of a result: what is read from the chapter on disk.
fn on_disk(result: &ResultItem) -> (Option<u32>, Option<&str>, Option<&str>) {
    (
        result.piece,
        result.name.as_deref(),
        result.caption.as_deref(),
    )
}

/// The `Send` bound is a proof at compile time that the window can run the ask on any thread of
/// its runtime.
async fn ask(cx: &LiveContext<StandInServices>) -> Vec<Event> {
    fn assert_send<F: Future + Send>(future: F) -> F {
        future
    }
    let draft = AskDraft {
        question: QUESTION.to_owned(),
        mode: AskMode::Answer,
        filters: Filters::default(),
    };
    let (reply, events, _stop) = Reply::collecting();
    assert_send(query::ask(cx, REQUEST, &draft, &reply)).await;
    events.try_iter().collect()
}
