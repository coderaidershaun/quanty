//! Draws the path in a test window and checks what a person reads: the steps, their counts and
//! what the panel says while it waits or when the search failed.

use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::{Local, show};
use crate::contract::{AskDraft, Event, Failure, FailureKind, Intent, RetrievalTrace, SearchReply};
use crate::state::Shared;
use crate::testkit::{self, Host, sample};
use crate::theme::space;

const DEFAULT: [f32; 2] = [420.0, 273.0];
const SMALLEST: [f32; 2] = [303.0, 220.0];

const TITLES: [&str; 5] = [
    "Nearest items",
    "Concepts matched",
    "One hop in the graph",
    "Ranked and capped",
    "Cited items",
];

fn path(size: [f32; 2], shared: Shared) -> Harness<'static, Host> {
    let mut local = Local::default();
    testkit::panel(size, shared, move |ui, cx| show(ui, &mut local, cx))
}

/// Whether some node of the window carries these words. A notice or a placeholder can name
/// itself with more than one node, so a words check never asks for exactly one.
fn says(harness: &Harness<'_, Host>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

/// Where a panel of this size sits in the test window: the whole of the card.
fn card(size: [f32; 2]) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(space::LG, space::LG), egui::Vec2::from(size))
}

fn names(names: &[&str]) -> Option<Vec<String>> {
    Some(names.iter().map(|name| (*name).to_owned()).collect())
}

/// A search that reached every step.
fn black_scholes() -> RetrievalTrace {
    RetrievalTrace {
        documents_searched: None,
        nearest: 8,
        seed_concepts: names(&[
            "Black–Scholes model",
            "Volatility",
            "Itô's lemma",
            "Risk-free rate",
            "European option",
        ]),
        related_concepts: names(&["Risk-neutral measure", "Hedging"]),
        candidates: Some(14),
        ranked: Some(14),
        kept: Some(8),
        passed_over: vec![(
            "Quanty Sample Notes, chapter 2: The Black–Scholes model".to_owned(),
            2,
        )],
        cited: names(&["Table 1-1"]),
    }
}

fn finished(trace: RetrievalTrace) -> Shared {
    testkit::searched(SearchReply {
        trace,
        ..sample::search_reply()
    })
}

#[test]
fn a_finished_search_shows_five_steps_with_its_own_names_and_counts_in_both_windows() {
    let lines = [
        "The stored items closest in meaning to the question.",
        "Black–Scholes model, Volatility, Itô's lemma and 2 more",
        "Risk-neutral measure, Hedging",
        "14 ranked together. The cap passed over 2: Quanty Sample Notes, chapter 2: The Black–Scholes model.",
        "Table 1-1",
    ];
    let badges = ["8 items", "5 concepts", "+6 items", "8 kept", "+1 item"];
    for (size, picture, has_lines) in [
        (DEFAULT, "retrieval-path-ready", true),
        (SMALLEST, "retrieval-path-ready-small", false),
    ] {
        let mut harness = path(size, finished(black_scholes()));
        harness.run();
        let card = card(size);

        let mut markers = Vec::new();
        for (index, title) in TITLES.iter().enumerate() {
            harness.get_by_label(title);
            let marker = harness.get_by_label(&format!("Step {}", index + 1)).rect();
            assert!(
                card.contains_rect(marker),
                "step {} left the card",
                index + 1
            );
            markers.push(marker);
        }
        assert!(
            harness.query_by_label("Step 6").is_none(),
            "a sixth row was drawn"
        );
        for pair in markers.windows(2) {
            assert!(pair[0].bottom() <= pair[1].top(), "two rows overlap");
        }
        for badge in badges {
            let node = harness.get_by_label(badge);
            assert!(card.contains_rect(node.rect()), "{badge} left the card");
        }
        for line in lines {
            let drawn = harness.query_all_by_label(line).next().is_some();
            assert_eq!(drawn, has_lines, "the line `{line}` at {size:?}");
        }
        testkit::save_png(&mut harness, picture);
    }

    // A search under a filter says how many documents it searched and what the filter kept out
    // of the ranking, and the cap names each document it passed over.
    let filtered = RetrievalTrace {
        documents_searched: Some(2),
        ranked: Some(11),
        kept: Some(6),
        passed_over: vec![("Notes".to_owned(), 3), ("Hull".to_owned(), 2)],
        ..black_scholes()
    };
    let mut harness = path(DEFAULT, finished(filtered));
    harness.run();
    harness.get_by_label("The closest items in the 2 documents with these labels.");
    harness.get_by_label(
        "11 of 14 ranked together; the others lack these labels. The cap passed over 5: 3 from Notes; 2 from Hull.",
    );
}

/// A search that found no item at all, as a search with no result reports itself.
fn found_nothing(trace: RetrievalTrace) -> Shared {
    testkit::searched(SearchReply {
        trace,
        ..SearchReply::default()
    })
}

#[test]
fn a_step_not_reached_says_so_and_a_real_zero_is_a_zero() {
    // The library holds nothing: the search stops after the first step.
    let mut harness = path(DEFAULT, found_nothing(RetrievalTrace::default()));
    harness.run();
    harness.get_by_label("0 items");
    harness.get_by_label("The library holds no items.");
    assert_eq!(harness.get_all_by_label("Not reached").count(), 4);
    assert!(!says(&harness, "0 concepts"), "a missing step drew a zero");
    assert!(!says(&harness, "+0"), "a missing step drew a zero");
    testkit::save_png(&mut harness, "retrieval-path-not-reached");

    // A filter that no document carries is not an empty library.
    let trace = RetrievalTrace {
        documents_searched: Some(0),
        ..RetrievalTrace::default()
    };
    let mut harness = path(DEFAULT, found_nothing(trace));
    harness.run();
    harness.get_by_label("0 documents");
    harness.get_by_label("No document has these labels. Nothing was searched.");
    assert!(!says(&harness, "The library holds no items"));
    assert_eq!(harness.get_all_by_label("Not reached").count(), 4);
    testkit::save_png(&mut harness, "retrieval-path-no-labels");

    // A filter that some documents carry, and no item of them is near: the filters are the cause.
    let trace = RetrievalTrace {
        documents_searched: Some(2),
        ..RetrievalTrace::default()
    };
    let mut harness = path(DEFAULT, found_nothing(trace));
    harness.run();
    harness.get_by_label("0 items");
    harness.get_by_label("No item matches the filters.");
    assert!(!says(&harness, "The library holds no items"));
    testkit::save_png(&mut harness, "retrieval-path-no-match");

    // Every step ran and found nothing to add: these are zeros, not steps that were missed.
    let trace = RetrievalTrace {
        nearest: 8,
        seed_concepts: Some(Vec::new()),
        related_concepts: Some(Vec::new()),
        candidates: Some(8),
        ranked: Some(8),
        kept: Some(8),
        cited: Some(Vec::new()),
        ..RetrievalTrace::default()
    };
    let mut harness = path(DEFAULT, found_nothing(trace));
    harness.run();
    harness.get_by_label("8 items");
    harness.get_by_label("0 concepts");
    assert_eq!(harness.get_all_by_label("+0 items").count(), 2);
    harness.get_by_label("8 kept");
    assert!(
        !says(&harness, "Not reached"),
        "a step that ran was called missed"
    );
    testkit::save_png(&mut harness, "retrieval-path-none-found");
}

#[test]
fn the_path_says_what_it_waits_for_and_what_went_wrong() {
    let mut harness = path(DEFAULT, Shared::default());
    harness.run();
    assert!(says(&harness, "No search yet"));
    assert!(says(
        &harness,
        "Ask a question to see how its results are found."
    ));
    testkit::save_png(&mut harness, "retrieval-path-empty");

    // A new ask never shows the steps of the one before it.
    let mut harness = path(DEFAULT, finished(black_scholes()));
    harness.run();
    harness.get_by_label("8 items");
    ask(&mut harness, "What is vega?");
    harness.run();
    for number in 1..=5 {
        harness.get_by_label(&format!("Step {number}"));
    }
    harness.get_by_role_and_label(Role::ProgressIndicator, "Searching");
    assert!(
        harness.query_all_by_label("8 items").next().is_none(),
        "the old counts stayed under the new question"
    );
    testkit::save_png(&mut harness, "retrieval-path-searching");

    let failure = sample::failure(FailureKind::QdrantDown);
    deliver(&mut harness, Err(failure.clone()));
    harness.run();
    assert!(says(&harness, "The search failed"));
    assert!(says(&harness, &failure.hint));
    assert!(harness.query_by_label("Searching").is_none());
    assert!(harness.query_by_label("Step 1").is_none());
    testkit::save_png(&mut harness, "retrieval-path-failed");

    ask(&mut harness, "What is theta?");
    harness
        .state_mut()
        .shared
        .apply_intent(Intent::CancelAsk, &mut Vec::new());
    harness.run();
    assert!(says(&harness, "Search stopped"));
    assert!(says(
        &harness,
        "Ask again to see how the results are found."
    ));
    assert!(harness.query_by_label("Searching").is_none());
    testkit::save_png(&mut harness, "retrieval-path-stopped");

    // A reply that is back before a frame drew the wait still replaces the old steps.
    let mut harness = path(DEFAULT, finished(black_scholes()));
    harness.run();
    ask(&mut harness, "What is vega?");
    let quick = SearchReply {
        trace: RetrievalTrace {
            nearest: 3,
            ..RetrievalTrace::default()
        },
        ..SearchReply::default()
    };
    deliver(&mut harness, Ok(quick));
    harness.run();
    harness.get_by_label("3 items");
    assert!(harness.query_all_by_label("8 items").next().is_none());
}

/// Asks a new question, as the app does after the Ask bar or Follow up sent it.
fn ask(harness: &mut Harness<'_, Host>, question: &str) {
    let draft = AskDraft {
        question: question.to_owned(),
        ..AskDraft::default()
    };
    harness
        .state_mut()
        .shared
        .apply_intent(Intent::Ask(draft), &mut Vec::new());
}

/// The reply of the search that is running.
fn deliver(harness: &mut Harness<'_, Host>, result: Result<SearchReply, Failure>) {
    let shared = &mut harness.state_mut().shared;
    let request = shared.ask.request.expect("an ask is running");
    shared.apply_event(Event::Search { request, result }, &mut Vec::new());
}
