//! Checks that an ask on the live backend finds the stored items and writes the answer.
// SMELL: this file is over 400 lines, and the limit is 500. The part to move out is the filling of
// the stores below the tests. `sample_chapter` and `copy_folder` are also written again in other
// tests of the live backend, so all of these want one shared home.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use graph::{ConceptNode, GraphStore, Mention, Relation, RelationKind};
use gui::backend::Reply;
use gui::backend::live::{LiveContext, query};
use gui::contract::{
    AnswerBlock, AskDraft, AskMode, ConceptId, DocId, EdgeKind, Event, Filters, GraphEdge,
    GraphNode, ItemId, ItemKind, NodeId, NodeKind, Reason, RequestId, ResultItem,
};
use ocr::testing::{Scenario, StubServices};
use rag_core::{ItemKind as StoredKind, LlmError};
use rag_ingestion::testing::{StandInEmbedder, StandInLlm, ThrowawayStores, first_axis, vector_at};
use rag_ingestion::{Item, chapter_items, ingest_chapter};
use rag_retrieval::RESULTS_PER_QUERY;
use serde_json::json;

use crate::support::StandInServices;

const QUESTION: &str = "How is a call option priced?";
const REQUEST: RequestId = RequestId(7);
const IN_DEPTH: &str = "quanty-sample-notes/chapter-2";
const INTUITION: &str = "quanty-sample-notes/chapter-1";
const SAMPLE_PAGES: &str = "option-volatility-and-pricing/chapter-1";
const TABLE_CAPTION: &str = "How the price of a call and of a put respond when one input rises.";

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
    let world = fill("query-ask", &answering).await;

    let events = ask(&world.cx, AskMode::Answer, Filters::default()).await;

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
    assert_eq!(formula.book.as_deref(), Some("Quanty Sample Notes"));
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

    let events = ask(&cx, AskMode::Answer, Filters::default()).await;

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
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p gui --test integration -- --ignored query::"]
async fn a_results_only_ask_obeys_its_filters_and_a_failed_answer_keeps_the_results() {
    let answering = StandInLlm::replying(|_, _| {
        Err(LlmError::UsageLimit {
            message: "the usage limit was reached".to_owned(),
        })
    });
    let world = fill("query-filters", &answering).await;
    let sample_notes = Filters {
        book: Some("quanty sample notes".to_owned()),
        tags: vec![" ".to_owned()],
        ..Filters::default()
    };

    let events = ask(&world.cx, AskMode::ResultsOnly, sample_notes.clone()).await;

    let [
        Event::Search {
            result: Ok(search), ..
        },
        Event::Graph { result: Ok(_), .. },
    ] = events.as_slice()
    else {
        panic!("expected the search and the graph, with no answer: {events:#?}");
    };
    assert!(!search.results.is_empty());
    assert!(
        (search.results.iter()).all(|result| result.book.as_deref() == Some("Quanty Sample Notes"))
    );
    assert_eq!(search.trace.documents_searched, Some(2));
    assert_eq!(answering.calls(), 0);

    let events = ask(&world.cx, AskMode::Answer, sample_notes).await;

    let [
        Event::Search { result: Ok(_), .. },
        Event::Graph { result: Ok(_), .. },
        Event::Answer { result: Err(_), .. },
    ] = events.as_slice()
    else {
        panic!("expected the results to stay when the answer fails: {events:#?}");
    };
    assert_eq!(answering.calls(), 1);

    let no_such_book = Filters {
        book: Some("No such book".to_owned()),
        ..Filters::default()
    };
    let events = ask(&world.cx, AskMode::Answer, no_such_book).await;

    let [
        Event::Search {
            result: Ok(search), ..
        },
    ] = events.as_slice()
    else {
        panic!("expected the search alone: {events:#?}");
    };
    assert!(search.results.is_empty());
    assert_eq!(search.trace.documents_searched, Some(0));
    assert_eq!(search.trace.seed_concepts, None);
    assert_eq!(answering.calls(), 1);
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

/// Asks the question and returns every event that was sent. The `Send` bound is a proof at
/// compile time that the window can run the ask on any thread of its runtime.
async fn ask(cx: &LiveContext<StandInServices>, mode: AskMode, filters: Filters) -> Vec<Event> {
    fn assert_send<F: Future + Send>(future: F) -> F {
        future
    }
    let draft = AskDraft {
        question: QUESTION.to_owned(),
        mode,
        filters,
    };
    let (reply, events, _stop) = Reply::collecting();
    assert_send(query::ask(cx, REQUEST, &draft, &reply)).await;
    events.try_iter().collect()
}

/// Four stored items, from nearest to the question to farthest.
#[derive(Clone)]
struct Picks {
    formula: Item,
    table: Item,
    chunk: Item,
    figure: Item,
}

impl Picks {
    fn of_the_samples() -> Picks {
        Picks {
            formula: pick(IN_DEPTH, StoredKind::Formula, Some("(2.4)")),
            table: pick(INTUITION, StoredKind::Table, Some("Table 1-1")),
            chunk: pick(IN_DEPTH, StoredKind::Chunk, None),
            figure: pick(SAMPLE_PAGES, StoredKind::Figure, Some("Figure 13-4")),
        }
    }

    /// An embedder that puts the question on the first axis and each pick at its own distance
    /// from it. Every other item gets a made-up vector that is near no pick.
    fn embedder(&self) -> StandInEmbedder {
        StandInEmbedder::default()
            .placing(QUESTION, first_axis())
            .placing(&self.formula.input.text, vector_at(0.9, 1))
            .placing(&self.table.input.text, vector_at(0.8, 2))
            .placing(&self.chunk.input.text, vector_at(0.7, 3))
            .placing(&self.figure.input.text, vector_at(0.6, 4))
    }
}

/// A committed chapter, as a path that does not depend on where the test runs from.
fn sample_chapter(chapter: &str) -> PathBuf {
    let folder = gui::testkit::samples_folder().join(chapter);
    std::fs::canonicalize(folder).expect("a committed chapter should exist")
}

fn pick(chapter: &str, kind: StoredKind, label: Option<&str>) -> Item {
    let read = ocr::read_chapter(&sample_chapter(chapter)).expect("the chapter should be read");
    chapter_items(&read)
        .into_iter()
        .find(|item| item.payload.kind == kind && item.payload.label.as_deref() == label)
        .unwrap_or_else(|| panic!("{chapter} should have a {kind:?} labelled {label:?}"))
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder should be made");
    for entry in std::fs::read_dir(from).expect("the folder should be listed") {
        let entry = entry.expect("an entry should be read");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("a file should be copied");
        }
    }
}

/// A context over `stores` whose answers come from `answering`.
fn context_with(
    stores: ThrowawayStores,
    answering: &StandInLlm,
    embedder: impl Fn() -> StandInEmbedder + Send + Sync + 'static,
) -> LiveContext<StandInServices> {
    let config = stores.config().clone();
    let answering = answering.clone();
    let services = StandInServices {
        stores,
        embedder: Box::new(embedder),
        llm: Box::new(move |_model| answering.clone()),
        pages: Arc::new(StubServices::new(Scenario::SampleChapter)),
    };
    LiveContext::new(config, services)
}

struct World {
    cx: LiveContext<StandInServices>,
    picks: Picks,
    black_scholes: ConceptNode,
    lemma: ConceptNode,
}

/// Ingests the three committed chapters into throwaway stores, with the picks placed near the
/// question, then writes by hand what no stand-in finds: two concepts, what mentions one of
/// them, and the relation between them. Notes chapter 2 gets the folder the graph keeps, Notes
/// chapter 1 gets a copy under the content folder only, and the other chapter gets neither.
async fn fill(name: &str, answering: &StandInLlm) -> World {
    let stores = ThrowawayStores::new(name);
    let connected = stores.connect().await;
    let picks = Picks::of_the_samples();
    let models = stores.models_with_embedder(picks.embedder(), StandInLlm::finding_nothing());
    for chapter in [SAMPLE_PAGES, INTUITION, IN_DEPTH] {
        ingest_chapter(&sample_chapter(chapter), &models, &connected)
            .await
            .expect("a sample chapter should be ingested");
    }

    let concept = |name: &str, definition: &str| ConceptNode {
        id: rag_core::ConceptId::random(),
        name: name.to_owned(),
        normalised_name: name.to_lowercase(),
        definition: definition.to_owned(),
    };
    let black_scholes = concept("Black–Scholes model", "The model of an option's price.");
    let lemma = concept("Itô's lemma", "How a function of a random walk changes.");
    let graph = &connected.graph;
    for concept in [&black_scholes, &lemma] {
        let stored = graph.upsert_concept(concept).await;
        stored.expect("a concept should be stored");
    }
    let mentioning = |item: &Item| Mention {
        item: item.id,
        concept: black_scholes.id,
        wording: black_scholes.name.clone(),
    };
    let mentions = [mentioning(&picks.formula), mentioning(&picks.chunk)];
    graph
        .add_mentions(&mentions)
        .await
        .expect("the mentions should be stored");
    let derived = Relation {
        from: black_scholes.id,
        to: lemma.id,
        kind: RelationKind::DerivedFrom,
        item: picks.formula.id,
    };
    graph
        .add_relations(&[derived])
        .await
        .expect("the relation should be stored");
    graph
        .set_chapter_folder(picks.formula.payload.doc_id, &sample_chapter(IN_DEPTH))
        .await
        .expect("the folder should be stored");
    let content = &stores.config().content_folder;
    copy_folder(
        &sample_chapter(INTUITION),
        &content.join("quanty-sample-notes/chapter-1"),
    );

    let placing = picks.clone();
    World {
        cx: context_with(stores, answering, move || placing.embedder()),
        picks,
        black_scholes,
        lemma,
    }
}
