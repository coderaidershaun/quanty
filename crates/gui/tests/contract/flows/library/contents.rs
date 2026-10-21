//! The Code, Formulas, Figures and Tables views of a document list every item of that kind with
//! the page it is printed on, each item reads its page in the Source panel, a folder that is not
//! found says so, and the Concepts view lists the concepts of the document and says when the graph
//! is down.

use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Command, DocId, Event, Failure, PieceKind, Tab};
use gui::state::SourceTarget;
use gui::testkit;

use super::{
    BLACK_SCHOLES, BOOK, HAWKES, NOTES, PAPER, SAMPLE_PAGES, documents_of, open_document, view,
};
use crate::flows::recording::{self, Seen};
use crate::flows::{
    Wheel, Window, are_stacked_in_order, click, has, node, says, scroll_to, shared,
};

const FALKORDB_URL: &str = "falkor://localhost:6379";

/// Serves every command with the fake, and changes each event of a contents load with `edit`.
struct Edited {
    fake: Fake,
    edit: fn(Event) -> Event,
}

impl Handler for Edited {
    async fn serve(&self, command: Command, reply: Reply) {
        if !matches!(command, Command::LoadChapterContents { .. }) {
            return self.fake.serve(command, reply).await;
        }
        let (collected, events, _stop) = Reply::collecting();
        self.fake.serve(command, collected).await;
        for event in events.try_iter() {
            reply.send((self.edit)(event));
        }
    }
}

/// Makes the listing of the sample chapter run over a page break, which no sample listing does.
fn cut_listing(event: Event) -> Event {
    let Event::ChapterContents {
        request,
        result: Ok(mut contents),
    } = event
    else {
        return event;
    };
    for item in &mut contents.items {
        if item.piece.kind == PieceKind::Code {
            item.last_page = Some(item.page + 1);
            item.last_printed_page = Some("8".to_owned());
        }
    }
    Event::ChapterContents {
        request,
        result: Ok(contents),
    }
}

fn graph_down(event: Event) -> Event {
    match event {
        Event::ChapterConcepts { request, .. } => Event::ChapterConcepts {
            request,
            result: Err(Failure::falkordb_down(FALKORDB_URL)),
        },
        other => other,
    }
}

fn open_library(scene: &str) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    click(&mut harness, Role::Tab, "Library");
    (harness, seen)
}

fn open_library_with(edit: fn(Event) -> Event) -> (Window, Seen) {
    let (mut harness, seen) =
        recording::open_with("black-scholes", DEFAULT_WINDOW, move |fake: Fake| Edited {
            fake,
            edit,
        });
    testkit::settle(&mut harness);
    click(&mut harness, Role::Tab, "Library");
    (harness, seen)
}

fn id_of(harness: &Window, label: &str) -> DocId {
    documents_of(harness)
        .into_iter()
        .find(|document| document.name().label() == label)
        .unwrap_or_else(|| panic!("no sample document is named `{label}`"))
        .id
}

fn loads_of(seen: &Seen, id: DocId) -> usize {
    seen.count(|command| matches!(command, Command::LoadChapterContents { doc, .. } if *doc == id))
}

fn count_of(harness: &Window, view: &str) -> Option<String> {
    node(harness, Role::Tab, view).value()
}

#[test]
fn each_view_lists_every_item_of_its_kind_in_reading_order_with_its_page() {
    let (mut harness, seen) = open_library("black-scholes");

    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    click(&mut harness, Role::Tab, "Code");
    assert!(
        has(&harness, Role::Label, "Listing 2.1"),
        "the tag is the printed label"
    );
    assert!(
        has(&harness, Role::Label, "p. 7"),
        "the page is the one printed"
    );
    assert!(says(&harness, "python"));
    assert!(says(&harness, "The call price in Python"));
    assert_eq!(count_of(&harness, "Code").as_deref(), Some("1"));

    click(&mut harness, Role::Tab, "Formulas");
    assert!(
        are_stacked_in_order(&harness, Role::Label, &["(2.1)", "(2.2)", "(2.3)", "(2.4)"]),
        "the formulas are in reading order"
    );
    assert!(says(&harness, "Black–Scholes call price"));
    scroll_to(&mut harness, view(), Wheel::Down, Role::Label, "(2.6)");

    click(&mut harness, Role::Tab, "Figures");
    assert!(says(&harness, "No figures in this document"));
    click(&mut harness, Role::Tab, "Tables");
    assert!(says(&harness, "No tables in this document"));

    open_document(&mut harness, BOOK, SAMPLE_PAGES);
    assert!(
        are_stacked_in_order(
            &harness,
            Role::Label,
            &["Figure 7-2", "Figure 7-3", "Table"]
        ),
        "the tables are in reading order"
    );
    assert!(has(&harness, Role::Label, "p. 100"));
    assert!(has(&harness, Role::Label, "p. 246"));

    click(&mut harness, Role::Tab, "Figures");
    assert!(are_stacked_in_order(
        &harness,
        Role::Label,
        &["Figure 13-4", "Figure 13-16"]
    ));
    assert!(says(&harness, "Dividend sensitivity."));
    // A figure card is taller than half the room, so the third is not laid out until it is
    // scrolled to, and its picture is not asked for.
    assert!(!has(&harness, Role::Label, "Figure 24-12"));
    scroll_to(
        &mut harness,
        view(),
        Wheel::Down,
        Role::Label,
        "Figure 24-12",
    );
    assert!(says(
        &harness,
        "FTSE 100 volatility surface. March 16. 2012."
    ));

    for title in [BLACK_SCHOLES, SAMPLE_PAGES] {
        let loads = loads_of(&seen, id_of(&harness, title));
        assert_eq!(loads, 1, "all the views of `{title}` share one load");
    }
}

#[test]
fn an_item_reads_its_page_in_ask_with_the_piece_shown() {
    let (mut harness, _seen) = open_library_with(cut_listing);
    let doc = id_of(&harness, BLACK_SCHOLES);

    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    click(&mut harness, Role::Tab, "Code");
    assert!(has(&harness, Role::Label, "pp. 7\u{2013}8"));
    click(&mut harness, Role::Button, "Read on pp. 7\u{2013}8");

    assert_eq!(shared(&harness).tab, Tab::Ask);
    let target = SourceTarget {
        doc,
        page: 3,
        piece: Some(11),
    };
    assert_eq!(shared(&harness).source.target, Some(target));
    let shown = shared(&harness).source.page.ready().map(|page| page.page);
    assert_eq!(shown, Some(3), "the first page of the span is loaded");
}

#[test]
fn the_concepts_view_lists_the_concepts_most_mentioned_first_and_says_when_the_graph_is_down() {
    let (mut harness, _seen) = open_library("black-scholes");
    open_document(&mut harness, BOOK, SAMPLE_PAGES);
    click(&mut harness, Role::Tab, "Concepts");
    assert!(
        are_stacked_in_order(
            &harness,
            Role::Label,
            &["Volatility", "Forward price", "Volatility surface"]
        ),
        "the concept that more pages list comes first"
    );
    assert!(says(
        &harness,
        "How much the price of the stock moves, as a yearly standard deviation of its returns."
    ));
    assert_eq!(count_of(&harness, "Concepts").as_deref(), Some("3"));

    // The stores-down scene has no catalogue to open a document from, so a wrapper makes the
    // graph fail instead.
    let (mut harness, seen) = open_library_with(graph_down);
    open_document(&mut harness, BOOK, SAMPLE_PAGES);
    click(&mut harness, Role::Tab, "Figures");
    assert!(
        has(&harness, Role::Label, "Figure 13-4"),
        "the disk needs no store"
    );
    click(&mut harness, Role::Tab, "Concepts");
    assert!(says(&harness, "The concepts could not be read"));
    assert!(says(
        &harness,
        "Start FalkorDB at falkor://localhost:6379, then try again."
    ));

    let id = id_of(&harness, SAMPLE_PAGES);
    let before = loads_of(&seen, id);
    click(&mut harness, Role::Button, "Try again");
    assert_eq!(loads_of(&seen, id), before + 1, "Try again loads once more");
    assert!(shared(&harness).contents.items.ready().is_some());
}

#[test]
fn a_document_whose_folder_is_missing_says_so_with_what_to_do() {
    let (mut harness, seen) = open_library("source-missing");
    open_document(&mut harness, BOOK, SAMPLE_PAGES);
    click(&mut harness, Role::Tab, "Code");
    assert!(says(&harness, "The pages of this document were not found"));
    assert!(says(
        &harness,
        "quanty does not know where this document's pages are"
    ));
    let id = id_of(&harness, SAMPLE_PAGES);
    assert_eq!(loads_of(&seen, id), 1);
    click(&mut harness, Role::Button, "Try again");
    assert_eq!(loads_of(&seen, id), 2, "Try again loads once more");
    click(&mut harness, Role::Tab, "Concepts");
    assert!(says(&harness, "No concepts were found in this document"));

    let (mut harness, _seen) = open_library("black-scholes");
    open_document(&mut harness, HAWKES, PAPER);
    click(&mut harness, Role::Tab, "Code");
    assert!(says(&harness, "The pages of this document were not found"));
}
