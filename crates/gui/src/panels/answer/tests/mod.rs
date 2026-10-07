//! The Answer pane drawn in a test window: found by label and role, never by the text of a
//! block. The data, the helpers and the tests of the written answer are here; the tests of
//! the states and of the lists are in the two children.

mod lists;
mod states;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::{Local, show};
use crate::contract::{
    Answer, AnswerBlock, AskDraft, AskMode, Event, Failure, FailureKind, Intent, ItemKind, Reason,
    RequestId, ResultItem, SearchReply,
};
use crate::state::Shared;
use crate::testkit::{self, Host, sample};
use crate::theme::space;

const WIDE: [f32; 2] = [988.0, 547.0];
const TALL: [f32; 2] = [988.0, 2000.0];
const SMALL: [f32; 2] = [716.0, 300.0];

const FORMULA: &str = r"C = S\,N(d_1) - K e^{-rT} N(d_2)";
const TABLE: &str = "| Input | Call |\n| --- | --- |\n| Price | rises |";

fn item(number: usize, kind: ItemKind, text: &str) -> ResultItem {
    ResultItem {
        number,
        kind,
        text: text.to_owned(),
        label: None,
        name: None,
        image: None,
        caption: None,
        reason: Reason::Nearest,
        ..sample::search_reply().results.remove(0)
    }
}

/// Five results: a formula, two passages, a figure and a table.
fn found() -> SearchReply {
    let formula = ResultItem {
        label: Some("(2.4)".to_owned()),
        ..item(1, ItemKind::Formula, FORMULA)
    };
    let volatility = ResultItem {
        reason: Reason::Concept("Volatility".to_owned()),
        ..item(3, ItemKind::Chunk, "Volatility is how far a price moves.")
    };
    let figure = ResultItem {
        label: Some("Figure 13-4".to_owned()),
        image: sample::search_reply()
            .results
            .into_iter()
            .find_map(|result| result.image),
        ..item(4, ItemKind::Figure, "Three spreads show the same profit.")
    };
    let table = ResultItem {
        label: Some("Table 1-1".to_owned()),
        reason: Reason::Cited {
            by: 2,
            label: "Table 1-1".to_owned(),
        },
        ..item(5, ItemKind::Table, TABLE)
    };
    SearchReply {
        results: vec![
            formula,
            item(2, ItemKind::Chunk, "Prices move at random."),
            volatility,
            figure,
            table,
        ],
        ..SearchReply::default()
    }
}

/// An answer that cites all five results and has a card for three of them.
fn written() -> Answer {
    let paragraph = |text: &str, cites: &[usize]| AnswerBlock::Paragraph {
        text: text.to_owned(),
        cites: cites.to_vec(),
    };
    Answer {
        title: Some("How a call is priced".to_owned()),
        blocks: vec![
            paragraph("A call is worth", &[1]),
            AnswerBlock::Item(1),
            AnswerBlock::Heading("Assumptions".to_owned()),
            paragraph("Prices move at random.", &[2, 3]),
            paragraph("Three spreads can look alike.", &[4]),
            AnswerBlock::Item(4),
            paragraph("The inputs are in the table.", &[5]),
            AnswerBlock::Item(5),
        ],
        follow_ups: Vec::new(),
    }
}

fn answered() -> Shared {
    testkit::answered(found(), sample::concept_graph(), written())
}

fn pane(size: [f32; 2], shared: Shared) -> Harness<'static, Host> {
    let mut local = Local::default();
    testkit::panel(size, shared, move |ui, cx| show(ui, &mut local, cx))
}

fn apply(shared: &mut Shared, intent: Intent) {
    shared.apply_intent(intent, &mut Vec::new());
}

fn deliver(shared: &mut Shared, event: impl FnOnce(RequestId) -> Event) {
    let request = shared.ask.request.expect("an ask is running");
    shared.apply_event(event(request), &mut Vec::new());
}

fn failure(kind: FailureKind) -> Failure {
    sample::failure(kind).with_hint("A hint that tells what to do.")
}

fn open_tab(harness: &mut Harness<'_, Host>, tab: &str) {
    harness.get_by_role_and_label(Role::Tab, tab).click();
    harness.run();
}

/// The numbers of the result cards and rows in the pane, in the order they are drawn.
fn shown_results(harness: &Harness<'_, Host>) -> Vec<usize> {
    (1..=5)
        .filter(|number| {
            let label = format!("Result {number}");
            harness.query_by_label(&label).is_some()
        })
        .collect()
}

/// Saves a picture of the pane as a person sees it at the default size, on `tab` when given.
/// A picture or a formula that is asked for is finished first.
fn look(name: &str, shared: Shared, tab: Option<&str>) {
    let mut harness = pane(WIDE, shared);
    harness.run();
    if let Some(tab) = tab {
        open_tab(&mut harness, tab);
    }
    harness.state_mut().media.run_pending();
    harness.run();
    testkit::save_png(&mut harness, name);
}

/// Clicks a card or a row on its own surface, in its padding. The middle of a card is often a
/// block of text, and a block of text takes the click for itself.
fn click_surface(harness: &mut Harness<'_, Host>, label: &str) {
    let at = harness.get_by_label(label).rect().left_top() + egui::vec2(space::XS, space::XS);
    harness.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        });
    }
    harness.run();
}

fn click_first(harness: &mut Harness<'_, Host>, label: &str) {
    harness
        .get_all_by_label(label)
        .next()
        .unwrap_or_else(|| panic!("no node named `{label}`"))
        .click();
    harness.run();
}

/// The number the pane asked to select, after the app has applied what the pane pushed.
fn selected_after(harness: &mut Harness<'_, Host>, asked: usize) -> Option<usize> {
    assert_eq!(harness.state().intents, [Intent::SelectResult(asked)]);
    harness.state_mut().apply_intents();
    harness.state().shared.ask.selected_result
}

fn says(harness: &Harness<'_, Host>, words: &str) -> bool {
    harness.query_all_by_label_contains(words).next().is_some()
}

/// The ask of a person who wants the results and no answer.
fn results_only() -> Shared {
    let mut shared = Shared::default();
    let draft = AskDraft {
        question: "What is a call?".to_owned(),
        mode: AskMode::ResultsOnly,
        ..AskDraft::default()
    };
    apply(&mut shared, Intent::Ask(draft));
    deliver(&mut shared, |request| Event::Search {
        request,
        result: Ok(found()),
    });
    shared
}

#[test]
fn an_answer_shows_its_title_its_text_and_a_card_for_each_item() {
    let mut harness = pane(TALL, answered());
    harness.run();
    assert!(says(&harness, "How a call is priced"), "the title");
    assert!(says(&harness, "Assumptions"), "the heading");
    harness.get_by_label(FORMULA);
    harness.get_by_label("(2.4)");
    assert!(
        harness
            .query_all_by_label_contains("Figure 13-4")
            .any(|node| node.accesskit_node().role() == Role::ProgressIndicator),
        "a picture that is still on its way shows a spinner"
    );
    harness.state_mut().media.run_pending();
    harness.run();
    harness.get_by_role_and_label(Role::Image, "Figure 13-4");
    assert_eq!(
        shown_results(&harness),
        [1, 4, 5],
        "a card for each item, none for a passage"
    );
    look("answer-ready", answered(), None);

    let untitled = Answer {
        title: None,
        ..written()
    };
    let shared = testkit::answered(found(), sample::concept_graph(), untitled);
    let question = shared.ask.question.clone();
    let mut harness = pane(WIDE, shared);
    harness.run();
    assert!(
        says(&harness, &question),
        "with no title the question stands in its place"
    );
}

#[test]
fn a_chip_a_card_and_a_row_select_their_result() {
    let mut harness = pane(TALL, answered());
    harness.run();
    click_first(&mut harness, "Citation 1");
    assert_eq!(
        selected_after(&mut harness, 1),
        Some(1),
        "a chip in the answer"
    );
    click_surface(&mut harness, "Result 1");
    assert_eq!(selected_after(&mut harness, 1), Some(1), "a formula card");
    click_surface(&mut harness, "Result 4");
    assert_eq!(selected_after(&mut harness, 4), Some(4), "a figure card");
    click_first(&mut harness, "Open in source");
    assert_eq!(
        selected_after(&mut harness, 4),
        Some(4),
        "the button of the figure card"
    );
    harness
        .get_all_by_label("Open in source")
        .nth(1)
        .expect("a button on the table card")
        .click();
    harness.run();
    assert_eq!(
        selected_after(&mut harness, 5),
        Some(5),
        "the button of the table card"
    );

    open_tab(&mut harness, "Results");
    click_surface(&mut harness, "Result 3");
    assert_eq!(selected_after(&mut harness, 3), Some(3), "a row");
    click_first(&mut harness, "Citation 2");
    assert_eq!(
        selected_after(&mut harness, 2),
        Some(2),
        "the chip of a row"
    );
}

#[test]
fn copy_latex_and_share_send_their_intents_and_share_waits_for_an_answer() {
    let mut waiting = pane(WIDE, testkit::searched(found()));
    waiting.run();
    waiting.get_by_role_and_label(Role::Button, "Share").click();
    waiting.run();
    assert!(
        waiting.state().intents.is_empty(),
        "no answer yet, nothing to share"
    );

    let mut harness = pane(TALL, answered());
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Share").click();
    harness.run();
    assert_eq!(harness.state().intents, [Intent::ShareAnswer]);
    harness.get_by_role_and_label(Role::Button, "Copied");
    harness.state_mut().apply_intents();

    click_first(&mut harness, "Copy LaTeX");
    assert_eq!(
        harness.state().intents,
        [Intent::CopyText(FORMULA.to_owned())]
    );
    let copied = |harness: &Harness<'_, Host>| {
        harness
            .query_all_by_role_and_label(Role::Button, "Copied")
            .count()
    };
    assert_eq!(
        copied(&harness),
        1,
        "Copy LaTeX reads Copied, and Share is a button again"
    );
    harness.hover_at(egui::pos2(1.0, 1.0));
    harness.run();
    assert_eq!(
        copied(&harness),
        0,
        "it goes back to its name when the pointer leaves"
    );
    harness.get_by_role_and_label(Role::Button, "Share");
}
