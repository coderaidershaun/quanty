//! Checks the whole app on the live backend: over throwaway stores that hold the sample
//! chapters, and over stores that are down.

use std::path::{Path, PathBuf};
use std::time::Duration;

use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use gui::app::App;
use gui::app::layout::{self, DEFAULT_WINDOW};
use gui::contract::{
    AskDraft, Failure, FailureKind, Filters, Intent, ItemKind, Loadable, NodeId, NodeKind,
    PieceKind, ResultItem, StartupFacts,
};
use gui::testkit;

use crate::support::{self, QUESTION, SAMPLE_PAGES};

type Window = Harness<'static, App>;

const SLOW: Duration = Duration::from_secs(30);
const VOLATILITY_BOOK: &str = "Option Volatility and Pricing";
const NOTES_BOOK: &str = "Quanty Sample Notes";
const PAGE_WITH_THE_FIGURE: u32 = 5;

#[test]
#[ignore = "needs local Qdrant and FalkorDB, bills nothing; run with: cargo test -p gui --test integration -- --ignored app::"]
fn an_ask_runs_from_the_question_to_the_cited_page_on_the_live_adapters() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime should start");
    let cx = runtime.block_on(support::seeded("app-ask"));
    let content_folder = cx.config().content_folder.clone();
    let mut harness = testkit::app_on(cx, facts(), Vec::new(), DEFAULT_WINDOW);
    testkit::settle_within(&mut harness, SLOW);

    the_media_list_offers_each_stored_book(&mut harness);
    let results = an_ask_brings_numbered_results_an_answer_and_a_graph(&mut harness);
    let mentioned = a_result_in_the_graph_selects_its_result(&mut harness);
    let figure = result_of(&results, ItemKind::Figure, "Figure 13-4");
    let formula = result_of(&results, ItemKind::Formula, "(2.4)");
    let table = result_of(&results, ItemKind::Table, "Table 1-1");
    assert_eq!(
        (formula.number, table.number, figure.number),
        (1, 2, 4),
        "the answer of the seed cites these three numbers"
    );
    a_cited_figure_opens_its_page_with_its_picture(&mut harness, figure);
    a_cited_formula_opens_its_page_with_its_concepts(&mut harness, formula, &mentioned);
    the_chapter_of_the_formula_is_found_in_the_content_folder(&harness, formula, &content_folder);
    a_cited_table_with_no_chapter_says_how_to_find_it(&mut harness, table);
    a_follow_up_asks_its_own_question(&mut harness);
    a_media_filter_keeps_the_results_of_that_book(&mut harness);
    a_media_that_is_not_stored_finds_nothing_and_says_so(&mut harness);
}

fn the_media_list_offers_each_stored_book(harness: &mut Window) {
    let catalogue = harness.state().shared().library.catalogue.ready();
    let documents = catalogue.map(|catalogue| catalogue.documents().count());
    assert_eq!(documents, Some(3));
    open_in_the_ask_bar(harness, "Media");
    for title in [VOLATILITY_BOOK, NOTES_BOOK] {
        harness.get_by_role_and_label(Role::Button, title);
    }
    harness
        .get_by_role_and_label(Role::Button, "All media")
        .click();
    harness.run_ok();
}

fn an_ask_brings_numbered_results_an_answer_and_a_graph(harness: &mut Window) -> Vec<ResultItem> {
    ask_in_the_box(harness, Some(QUESTION));
    let shared = harness.state().shared();
    let results = shared.ask.results().to_vec();
    let numbers: Vec<usize> = results.iter().map(|result| result.number).collect();
    assert_eq!(numbers, (1..=results.len()).collect::<Vec<_>>());
    assert!(
        shared.ask.answer.ready().is_some(),
        "{:?}",
        shared.ask.answer
    );
    let graph = shared.ask.graph.ready().expect("the graph should arrive");
    assert!(
        graph.nodes.len() <= 20,
        "the graph has {} nodes",
        graph.nodes.len()
    );
    harness
        .get_all_by_label("Citation 1")
        .next()
        .expect("a citation chip");
    results
}

/// Returns the name of a concept that the graph holds.
fn a_result_in_the_graph_selects_its_result(harness: &mut Window) -> String {
    let graph = (harness.state().shared().ask.graph.ready()).expect("the graph should arrive");
    let concept = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Concept)
        .expect("the hand-written concepts should be in the graph");
    let mentioned = concept.label.clone();
    harness.get_by_label(&format!("Concept {mentioned}"));
    let of_a_result = graph
        .nodes
        .iter()
        .find_map(|node| match node.id {
            NodeId::Item(number) => Some((number, format!("{}, result {number}", node.label))),
            NodeId::Concept(_) => None,
        })
        .expect("a result should be in the graph");
    harness.get_by_label(&of_a_result.1).click();
    settle(harness);
    assert_eq!(
        harness.state().shared().ask.selected_result,
        Some(of_a_result.0)
    );
    mentioned
}

fn a_cited_figure_opens_its_page_with_its_picture(harness: &mut Window, figure: &ResultItem) {
    cite(harness, figure);
    let shared = harness.state().shared();
    let piece = figure
        .piece
        .expect("the figure's chapter has its folder stored");
    let view = shared
        .source
        .page
        .ready()
        .expect("the figure's page should load");
    assert_eq!((view.doc, view.page), (figure.doc, figure.page));
    assert_eq!(target_piece(harness), Some(piece));
    assert!(
        shared.source.concepts.ready().is_some(),
        "{:?}",
        shared.source.concepts
    );
    harness.get_by_label(&format!("Picture of page {}", figure.page));
    let name = figure.label.as_deref().expect("the figure has a label");
    harness.get_by_label(&format!("{name} on the page"));
    let on_the_page = view
        .pieces
        .iter()
        .find(|candidate| candidate.number == piece);
    let on_the_page = on_the_page.expect("the page should have the piece of the result");
    assert_eq!(on_the_page.kind, PieceKind::Figure);
    let picture = figure.image.as_ref().map(|image| &image.path);
    assert!(picture.is_some(), "the figure result should have a picture");
    assert_eq!(
        picture,
        on_the_page.image.as_ref().map(|image| &image.path),
        "the result and the page must name one picture file"
    );
}

fn a_cited_formula_opens_its_page_with_its_concepts(
    harness: &mut Window,
    formula: &ResultItem,
    mentioned: &str,
) {
    cite(harness, formula);
    let shared = harness.state().shared();
    let piece = formula
        .piece
        .expect("the content folder's chapter should be found by its id");
    let view = shared
        .source
        .page
        .ready()
        .expect("the formula's page should load");
    assert_eq!((view.doc, view.page), (formula.doc, formula.page));
    assert_eq!(target_piece(harness), Some(piece));
    let marked = view
        .pieces
        .iter()
        .find(|candidate| candidate.number == piece);
    let marked = marked.expect("the page should have the piece of the result");
    assert_eq!(marked.kind, PieceKind::Formula);
    assert_eq!(marked.label, formula.label);
    assert_eq!(marked.name, formula.name);
    let on_the_page =
        (shared.source.concepts.ready()).expect("the concepts of the formula's page should load");
    assert!(
        on_the_page.iter().any(|concept| concept.name == mentioned),
        "the formula mentions {mentioned}: {on_the_page:?}"
    );
}

/// Only a scan of the content folder finds this chapter, since the graph keeps no folder.
fn the_chapter_of_the_formula_is_found_in_the_content_folder(
    harness: &Window,
    formula: &ResultItem,
    content_folder: &Path,
) {
    let shared = harness.state().shared();
    let folder = (shared.library.catalogue.ready())
        .and_then(|catalogue| catalogue.document(formula.doc))
        .and_then(|document| document.folder.clone())
        .expect("the catalogue should give the chapter a folder");
    assert!(folder.starts_with(content_folder), "{folder:?}");
    assert_eq!(support::document_of(&folder), formula.doc);
}

/// No folder is stored and no scan finds this chapter.
fn a_cited_table_with_no_chapter_says_how_to_find_it(harness: &mut Window, table: &ResultItem) {
    cite(harness, table);
    let shared = harness.state().shared();
    assert_eq!(table.piece, None);
    let failure = shared
        .source
        .page
        .failure()
        .expect("the page cannot be opened");
    assert_eq!(failure.kind, FailureKind::SourceMissing);
    assert!(failure.hint.contains("rag-ingest"), "{}", failure.hint);
    assert!(says(harness, "rag-ingest"));
    harness.get_by_role_and_label(Role::Button, "Try again");
}

fn a_follow_up_asks_its_own_question(harness: &mut Window) {
    let shared = harness.state().shared();
    let generation = shared.ask.generation;
    let follow_up = (shared.ask.answer.ready())
        .and_then(|answer| answer.follow_ups.first())
        .expect("the answer should suggest a follow-up")
        .clone();
    harness.get_by_label(&follow_up).click();
    settle(harness);
    let ask = &harness.state().shared().ask;
    assert_eq!(ask.generation, generation + 1);
    assert_eq!(ask.question, follow_up);
    assert!(ask.search.ready().is_some() && ask.graph.ready().is_some());
    assert!(ask.answer.ready().is_some());
}

fn a_media_filter_keeps_the_results_of_that_book(harness: &mut Window) {
    choose(harness, "Media", VOLATILITY_BOOK);
    choose(harness, "Mode", "Results only");
    ask_in_the_box(harness, None);
    let ask = &harness.state().shared().ask;
    let reply = ask.search.ready().expect("the search should arrive");
    assert!(!reply.results.is_empty());
    assert!(
        reply
            .results
            .iter()
            .all(|result| result.media.as_deref() == Some(VOLATILITY_BOOK))
    );
    assert_eq!(reply.trace.documents_searched, Some(1));
    assert_eq!(ask.answer, Loadable::Idle);
}

fn a_media_that_is_not_stored_finds_nothing_and_says_so(harness: &mut Window) {
    let no_such_book = AskDraft {
        question: QUESTION.to_owned(),
        filters: Filters {
            media: Some("No such book".to_owned()),
            ..Filters::default()
        },
        ..AskDraft::default()
    };
    harness.state_mut().push(Intent::Ask(no_such_book));
    settle(harness);
    let reply = (harness.state().shared().ask.search.ready()).expect("the search should arrive");
    assert!(reply.results.is_empty());
    assert_eq!(reply.trace.documents_searched, Some(0));
    assert!(says(harness, "No document has these labels"));
    assert!(!says(harness, "No sources found"));
}

#[test]
fn the_app_opens_with_both_stores_down_and_says_what_to_start() {
    let samples = testkit::samples_folder();
    let mut harness = testkit::app_on(
        support::closed(&samples),
        facts(),
        Vec::new(),
        DEFAULT_WINDOW,
    );
    testkit::settle_within(&mut harness, Duration::from_secs(10));

    let catalogue = &harness.state().shared().library.catalogue;
    let failure = catalogue
        .failure()
        .expect("the catalogue cannot load with the stores down");
    assert!(names_a_store(failure), "{failure:?}");
    assert!(says(&harness, &failure.hint));
    harness.get_by_role_and_label(Role::Button, "Try again");

    let chapter = support::sample_chapter(SAMPLE_PAGES);
    let doc = support::document_of(&chapter);
    harness.state_mut().push(Intent::OpenSource {
        doc,
        page: PAGE_WITH_THE_FIGURE,
        piece: None,
    });
    testkit::settle_within(&mut harness, Duration::from_secs(10));
    let source = &harness.state().shared().source;
    let shown = source.page.ready().map(|view| (view.doc, view.page));
    assert_eq!(
        shown,
        Some((doc, PAGE_WITH_THE_FIGURE)),
        "{:?}",
        source.page
    );
    let concepts = source
        .concepts
        .failure()
        .expect("the concepts need the graph");
    assert!(names_a_store(concepts), "{concepts:?}");
    harness.get_by_label(&format!("Picture of page {PAGE_WITH_THE_FIGURE}"));
}

fn facts() -> StartupFacts {
    StartupFacts {
        home: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        env_file: None,
        fixture: None,
        anthropic_api_key_set: false,
    }
}

fn settle(harness: &mut Window) {
    testkit::settle_within(harness, SLOW);
}

fn target_piece(harness: &Window) -> Option<u32> {
    let target = harness.state().shared().source.target.as_ref();
    target.and_then(|target| target.piece)
}

fn names_a_store(failure: &Failure) -> bool {
    matches!(
        failure.kind,
        FailureKind::QdrantDown | FailureKind::FalkorDbDown
    )
}

fn says(harness: &Window, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

fn ask_in_the_box(harness: &mut Window, question: Option<&str>) {
    harness.get_by_label("Question").click();
    if let Some(question) = question {
        harness.get_by_label("Question").type_text(question);
    }
    harness.run_ok();
    harness.key_press(egui::Key::Enter);
    settle(harness);
}

/// The Ask bar and the Source panel each have a list named "Media", so the list is found by the
/// panel it is in.
fn open_in_the_ask_bar(harness: &mut Window, list: &str) {
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, DEFAULT_WINDOW.into());
    let bar = layout::shell(window).ask_bar;
    harness
        .query_all_by_role_and_label(Role::ComboBox, list)
        .find(|node| bar.contains(node.rect().center()))
        .unwrap_or_else(|| panic!("no list named `{list}` is in the Ask bar"))
        .click();
    harness.run_ok();
}

fn choose(harness: &mut Window, list: &str, row: &str) {
    open_in_the_ask_bar(harness, list);
    harness.get_by_role_and_label(Role::Button, row).click();
    settle(harness);
}

fn cite(harness: &mut Window, result: &ResultItem) {
    let name = format!("Citation {}", result.number);
    harness
        .get_all_by_label(&name)
        .next()
        .expect("a citation chip")
        .click();
    settle(harness);
    assert_eq!(
        harness.state().shared().ask.selected_result,
        Some(result.number)
    );
}

fn result_of<'a>(results: &'a [ResultItem], kind: ItemKind, label: &str) -> &'a ResultItem {
    let found = results
        .iter()
        .find(|result| result.kind == kind && result.label.as_deref() == Some(label));
    found.unwrap_or_else(|| panic!("the results should hold the {kind:?} {label}"))
}
