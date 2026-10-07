//! Draws Follow up in a test window and checks what a person reads under the box and what a
//! question from here sends.

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::{Local, show};
use crate::contract::{
    Answer, AnswerBlock, AskDraft, AskMode, Event, FailureKind, Filters, Intent, RequestId,
    RetrievalTrace, SearchReply,
};
use crate::state::Shared;
use crate::testkit::{self, Host, sample};
use crate::theme::space;

const DEFAULT: [f32; 2] = [504.0, 273.0];
const SMALLEST: [f32; 2] = [420.0, 220.0];

const QUESTIONS: [&str; 4] = [
    "How does volatility change the Black–Scholes price of a call option?",
    "What is the risk-neutral measure and why is it used to price options?",
    "How is the Black–Scholes partial differential equation derived?",
    "What does put–call parity say about European options?",
];

fn follow_up(size: [f32; 2], shared: Shared) -> Harness<'static, Host> {
    let mut local = Local::default();
    testkit::panel(size, shared, move |ui, cx| show(ui, &mut local, cx))
}

/// Whether some node of the window carries these words. A notice or a placeholder can name
/// itself with more than one node, so a words check never asks for exactly one.
fn says(harness: &Harness<'_, Host>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

fn deliver(shared: &mut Shared, event: impl FnOnce(RequestId) -> Event) {
    let request = shared.ask.request.expect("an ask was made");
    shared.apply_event(event(request), &mut Vec::new());
}

fn apply(shared: &mut Shared, intent: Intent) {
    shared.apply_intent(intent, &mut Vec::new());
}

/// An ask in this mode, with its search still running.
fn asked_in(mode: AskMode) -> Shared {
    let mut shared = Shared::default();
    let draft = AskDraft {
        question: "How is vega defined?".to_owned(),
        mode,
        ..AskDraft::default()
    };
    apply(&mut shared, Intent::Ask(draft));
    shared
}

/// The search is back with results and the answer is being written.
fn answering() -> Shared {
    testkit::searched(sample::search_reply())
}

/// The whole ask came back with this answer.
fn answered_with(answer: Answer) -> Shared {
    testkit::answered(sample::search_reply(), sample::concept_graph(), answer)
}

fn paragraph() -> Vec<AnswerBlock> {
    vec![AnswerBlock::Paragraph {
        text: "Vega is the change in price for a change in volatility.".to_owned(),
        cites: vec![1],
    }]
}

/// What a question from this panel must carry: the book and tag of the ask on screen.
fn filters() -> Filters {
    let book = sample::catalogue().books[0].title.clone();
    Filters {
        book: Some(book.expect("the first sample book has a title")),
        author: None,
        tags: vec!["options".to_owned()],
    }
}

/// An answered ask under a book and a tag, with the four suggestions.
fn suggested() -> Shared {
    let mut shared = answered_with(Answer {
        title: None,
        blocks: paragraph(),
        follow_ups: QUESTIONS.map(str::to_owned).to_vec(),
    });
    shared.ask.filters = filters();
    shared
}

/// The inside of a panel of this size: where the card keeps its content.
fn inside(size: [f32; 2]) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(space::LG, space::LG), egui::Vec2::from(size))
        .shrink(space::LG)
}

#[test]
fn a_suggested_or_typed_question_starts_a_new_ask_with_the_same_mode_and_filters() {
    let mut harness = follow_up(DEFAULT, suggested());
    harness.run();
    harness.get_by_label("Each question is a new search");
    testkit::save_png(&mut harness, "follow-up-ready");
    let asked = |question: &str, mode: AskMode| {
        Intent::Ask(AskDraft {
            question: question.to_owned(),
            mode,
            filters: filters(),
        })
    };

    harness.get_by_label(QUESTIONS[2]).click();
    harness.run();
    harness.get_by_label("Ask follow-up").click();
    harness.run();
    let box_ = harness.get_by_label("Follow-up question");
    box_.focus();
    harness.run();
    harness
        .get_by_label("Follow-up question")
        .type_text("  What is vega?  ");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().intents,
        vec![
            asked(QUESTIONS[2], AskMode::Answer),
            asked("What is vega?", AskMode::Answer)
        ],
        "a chip and the box each ask once, and a blank box asks nothing"
    );
    harness.state_mut().apply_intents();
    harness.run();
    assert!(
        harness
            .get_by_label("Follow-up question")
            .value()
            .unwrap_or_default()
            .is_empty(),
        "the typed question stayed in the box for the next ask"
    );

    // A question typed while only results were asked for keeps that mode.
    let mut results_only = asked_in(AskMode::ResultsOnly);
    results_only.ask.filters = filters();
    let mut harness = follow_up(DEFAULT, results_only);
    harness.get_by_label("Follow-up question").focus();
    harness.run();
    harness
        .get_by_label("Follow-up question")
        .type_text("What is rho?");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().intents,
        vec![asked("What is rho?", AskMode::ResultsOnly)]
    );

    // At the smallest window all four suggestions are still on the card, one under the other.
    let mut harness = follow_up(SMALLEST, suggested());
    harness.run();
    let mut chips = Vec::new();
    for question in QUESTIONS {
        let chip = harness.get_by_label(question).rect();
        assert!(inside(SMALLEST).contains_rect(chip), "`{question}` is cut");
        chips.push(chip);
    }
    for pair in chips.windows(2) {
        assert!(pair[0].bottom() <= pair[1].top(), "two chips overlap");
    }
    testkit::save_png(&mut harness, "follow-up-ready-small");
}

/// A search that found no result, after a filter that matched this many documents.
fn found_nothing(documents_searched: Option<usize>) -> Shared {
    testkit::searched(SearchReply {
        results: Vec::new(),
        trace: RetrievalTrace {
            documents_searched,
            ..RetrievalTrace::default()
        },
    })
}

#[test]
fn no_sources_names_its_cause_and_what_to_do() {
    let unanswered = || Answer {
        title: None,
        blocks: Vec::new(),
        follow_ups: Vec::new(),
    };
    let cases = [
        (
            "follow-up-no-labels",
            found_nothing(Some(0)),
            "No document has these labels. Nothing was searched. Clear a filter in the Ask bar and ask again.",
        ),
        (
            "follow-up-labels-no-items",
            found_nothing(Some(2)),
            "No sources found. No item in your library matches the filters of this question. Clear a filter, then ask again.",
        ),
        (
            "follow-up-no-sources",
            found_nothing(None),
            "No sources found. Your library holds no items yet. Add a chapter on the Ingest tab, then ask again.",
        ),
        (
            "follow-up-no-answer-bare",
            answered_with(unanswered()),
            "No answer in the sources. The results do not answer this question. Ask it another way, or add a chapter that covers it.",
        ),
    ];
    for (picture, shared, notice) in cases {
        let mut harness = follow_up(DEFAULT, shared);
        harness.run();
        assert!(says(&harness, notice), "{picture} lacks `{notice}`");
        harness.get_by_label("Follow-up question");
        for part_not_built in ["Ingest a chapter", "Go to Ingest", "Open Ingest"] {
            assert!(
                !says(&harness, part_not_built),
                "{picture} offers `{part_not_built}`, which is not built"
            );
        }
        assert_eq!(
            harness.query_all_by_role(Role::Button).count(),
            1,
            "{picture}: a notice has a button, and only Ask follow-up is one"
        );
        testkit::save_png(&mut harness, picture);
    }

    // Asking for results only does not hide the cause.
    let mut results_only = asked_in(AskMode::ResultsOnly);
    deliver(&mut results_only, |request| Event::Search {
        request,
        result: Ok(SearchReply::default()),
    });
    let mut harness = follow_up(DEFAULT, results_only);
    harness.run();
    assert!(says(&harness, "No sources found. Your library holds no"));

    // An answer with no text still offers the questions the sources may answer.
    let mut shared = answered_with(Answer {
        follow_ups: QUESTIONS.map(str::to_owned).to_vec(),
        ..unanswered()
    });
    shared.ask.filters = filters();
    let mut harness = follow_up(DEFAULT, shared);
    harness.run();
    assert!(says(
        &harness,
        "The sources do not answer this. Questions they may answer:"
    ));
    for question in QUESTIONS {
        harness.get_by_label(question);
    }
    testkit::save_png(&mut harness, "follow-up-no-answer");
}

/// Scaffold: a theme with rows taller than a chip must not push the fourth chip off the card.
#[test]
fn the_fourth_chip_still_fits_when_a_theme_makes_rows_taller() {
    let mut local = Local::default();
    let mut harness = testkit::panel(SMALLEST, suggested(), move |ui, cx| {
        ui.spacing_mut().interact_size.y = 28.0;
        show(ui, &mut local, cx);
    });
    harness.run();
    let chip = harness.get_by_label(QUESTIONS[3]).rect();
    assert!(
        inside(SMALLEST).contains_rect(chip),
        "the fourth chip is cut"
    );
}

#[test]
fn every_state_with_no_suggestion_says_why() {
    let mut harness = follow_up(DEFAULT, Shared::default());
    harness.run();
    assert!(says(&harness, "Nothing to follow up yet"));
    assert!(says(
        &harness,
        "Ask a question above. Questions to ask next appear here."
    ));
    assert!(
        harness.query_by_label("Follow-up question").is_none(),
        "a box was drawn before the first ask"
    );
    testkit::save_png(&mut harness, "follow-up-empty");

    let mut stopped = testkit::asked("How is vega defined?");
    apply(&mut stopped, Intent::CancelAsk);
    let mut results_only = asked_in(AskMode::ResultsOnly);
    deliver(&mut results_only, |request| Event::Search {
        request,
        result: Ok(sample::search_reply()),
    });
    let failure = sample::failure(FailureKind::QdrantDown);
    let mut search_failed = testkit::asked("How is vega defined?");
    deliver(&mut search_failed, |request| Event::Search {
        request,
        result: Err(failure.clone()),
    });
    let usage_limit = sample::failure(FailureKind::ClaudeUsageLimit);
    let mut answer_failed = answering();
    deliver(&mut answer_failed, |request| Event::Answer {
        request,
        result: Err(usage_limit.clone()),
    });
    let mut answer_stopped = answering();
    apply(&mut answer_stopped, Intent::CancelAsk);
    let nothing_more = answered_with(Answer {
        title: None,
        blocks: paragraph(),
        follow_ups: Vec::new(),
    });
    let cases = [
        (
            "follow-up-search-stopped",
            stopped,
            vec!["The search was stopped, so there is nothing to follow up."],
        ),
        (
            "follow-up-searching",
            testkit::asked("How is vega defined?"),
            vec!["Suggestions come with the answer."],
        ),
        (
            "follow-up-results-only",
            results_only,
            vec![
                "Results only: no answer is written, so no questions are suggested. Choose Answer in the Ask bar to get them.",
            ],
        ),
        (
            "follow-up-search-failed",
            search_failed,
            vec![
                "The search failed, so there is nothing to follow up.",
                failure.hint.as_str(),
            ],
        ),
        (
            "follow-up-answering",
            answering(),
            vec!["Suggestions come with the answer."],
        ),
        (
            "follow-up-answer-failed",
            answer_failed,
            vec![
                "The answer was not written, so no questions were suggested.",
                usage_limit.hint.as_str(),
            ],
        ),
        (
            "follow-up-answer-stopped",
            answer_stopped,
            vec!["The answer was stopped, so no questions were suggested."],
        ),
        (
            "follow-up-none-offered",
            nothing_more,
            vec!["No further question was suggested for this answer."],
        ),
    ];
    for (picture, shared, words) in cases {
        let mut harness = follow_up(DEFAULT, shared);
        harness.run();
        for text in words {
            assert!(says(&harness, text), "{picture} lacks `{text}`");
        }
        harness.get_by_label("Follow-up question");
        testkit::save_png(&mut harness, picture);
    }

    // The wait for suggestions has a spinner. A mode that never gets suggestions has none.
    let mut harness = follow_up(DEFAULT, testkit::asked("How is vega defined?"));
    harness.run();
    harness.get_by_role_and_label(Role::ProgressIndicator, "Waiting for suggestions");
    let mut harness = follow_up(DEFAULT, asked_in(AskMode::ResultsOnly));
    harness.run();
    assert!(says(&harness, "Results only"));
    assert!(
        harness
            .query_by_role_and_label(Role::ProgressIndicator, "Waiting for suggestions")
            .is_none(),
        "a spinner waits for suggestions that never come"
    );
}
