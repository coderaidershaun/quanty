//! An item of the Code, Formulas, Figures and Tables views reads its page in the Source panel, a
//! folder that is not found says so, and the Concepts view lists the concepts of the document and
//! says when the graph is down.

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{Command, DocId, Event, Failure, PieceKind, Tab};
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::state::SourceTarget;
use gui::testkit;

use super::{BLACK_SCHOLES, BOOK, HAWKES, NOTES, PAPER, SAMPLE_PAGES, documents_of, open_document};
use crate::flows::recording::{self, Seen};
use crate::flows::{Window, are_stacked_in_order, click, copied_by, has, node, says, shared};

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
    let original = shared(&harness)
        .source
        .page
        .ready()
        .unwrap()
        .pieces
        .iter()
        .find(|piece| piece.number == 11)
        .unwrap();
    assert_eq!(
        original.caption.as_deref(),
        Some("The call price in Python"),
        "Source preserves the printed caption"
    );
}

#[test]
fn the_copy_icon_copies_a_library_item() {
    let (mut harness, _seen) = open_library("black-scholes");
    open_document(&mut harness, NOTES, BLACK_SCHOLES);
    click(&mut harness, Role::Tab, "Formulas");
    let first_formula = shared(&harness)
        .contents
        .items
        .ready()
        .and_then(|contents| contents.of_kind(PieceKind::Formula).next())
        .map(|item| item.piece.text.clone())
        .expect("the chapter has a formula");

    let copied = copied_by(&mut harness, |harness| {
        node(harness, Role::Button, "Copy LaTeX").click();
    });

    assert_eq!(copied, [first_formula]);
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
