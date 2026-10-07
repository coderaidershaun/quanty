//! The pane in each state of the search and of the answer, and from one ask to the next.

use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::{
    TALL, WIDE, answered, apply, click_first, click_surface, deliver, failure, found, open_tab,
    pane, results_only, says, selected_after, shown_results, written,
};
use crate::contract::{
    Answer, AskDraft, AskMode, Catalogue, Event, FailureKind, Intent, Loadable, RetrievalTrace,
    SearchReply,
};
use crate::state::Shared;
use crate::testkit::{self, Host};

/// What the pane says in a state, and whether the results stand under it.
struct State {
    name: &'static str,
    shared: Shared,
    words: [&'static str; 2],
    results_stand: bool,
}

#[test]
fn each_state_says_what_happened_and_what_to_do() {
    let mut empty_library = Shared::default();
    empty_library.library.catalogue = Loadable::Ready(Catalogue::default());

    let mut search_failed = testkit::asked("What is a call?");
    deliver(&mut search_failed, |request| Event::Search {
        request,
        result: Err(failure(FailureKind::QdrantDown)),
    });

    let mut search_stopped = testkit::asked("What is a call?");
    apply(&mut search_stopped, Intent::CancelAsk);

    let no_labels = testkit::searched(SearchReply {
        trace: RetrievalTrace {
            documents_searched: Some(0),
            ..RetrievalTrace::default()
        },
        ..SearchReply::default()
    });

    let mut answer_failed = testkit::searched(found());
    deliver(&mut answer_failed, |request| Event::Answer {
        request,
        result: Err(failure(FailureKind::ClaudeUsageLimit)),
    });

    let mut answer_empty = testkit::searched(found());
    deliver(&mut answer_empty, |request| Event::Answer {
        request,
        result: Ok(Answer::default()),
    });

    let mut answer_stopped = testkit::searched(found());
    apply(&mut answer_stopped, Intent::CancelAsk);

    let states = [
        State {
            name: "idle",
            shared: Shared::default(),
            words: [
                "Ask your books",
                "The answer comes with the pages it stands on.",
            ],
            results_stand: false,
        },
        State {
            name: "idle-empty-library",
            shared: empty_library,
            words: ["Your library is empty", "Add a chapter with rag-ingest"],
            results_stand: false,
        },
        State {
            name: "searching",
            shared: testkit::asked("What is a call?"),
            words: ["Searching your library", "The answer follows."],
            results_stand: false,
        },
        State {
            name: "search-failed",
            shared: search_failed,
            words: ["The search failed", "A hint that tells what to do."],
            results_stand: false,
        },
        State {
            name: "search-stopped",
            shared: search_stopped,
            words: ["Search stopped", "Ask again when you are ready."],
            results_stand: false,
        },
        State {
            name: "no-labels",
            shared: no_labels,
            words: ["No document has these labels", "Change or remove a filter"],
            results_stand: false,
        },
        State {
            name: "no-results",
            shared: testkit::searched(SearchReply::default()),
            words: [
                "No results",
                "Nothing in your library is close to this question.",
            ],
            results_stand: false,
        },
        State {
            name: "writing",
            shared: testkit::searched(found()),
            words: ["Writing the answer", "The results below are ready now."],
            results_stand: true,
        },
        State {
            name: "failed",
            shared: answer_failed,
            words: [
                "The answer could not be written",
                "A hint that tells what to do.",
            ],
            results_stand: true,
        },
        State {
            name: "empty",
            shared: answer_empty,
            words: [
                "The results do not answer the question",
                "Try a follow-up question.",
            ],
            results_stand: true,
        },
        State {
            name: "stopped",
            shared: answer_stopped,
            words: ["Answer stopped", "Ask again for the answer."],
            results_stand: true,
        },
        State {
            name: "results-only",
            shared: results_only(),
            words: [
                "No answer was asked for",
                "Choose Answer beside the question",
            ],
            results_stand: true,
        },
    ];
    for State {
        name,
        shared,
        words,
        results_stand,
    } in states
    {
        // A results-only ask opens on the Results tab, which has no words to say.
        let opens_on_results = shared.ask.mode == AskMode::ResultsOnly;
        let mut harness = pane(WIDE, shared);
        harness.run();
        if opens_on_results {
            open_tab(&mut harness, "Answer");
        }
        for text in words {
            assert!(says(&harness, text), "{name} does not say `{text}`");
        }
        assert_eq!(
            harness.query_by_label("Result 1").is_some(),
            results_stand,
            "{name}: do the results stand under the words?"
        );
        let tabs_are_off = harness
            .get_by_role_and_label(Role::Tab, "Results")
            .accesskit_node()
            .is_disabled();
        assert_eq!(
            tabs_are_off, !results_stand,
            "{name}: the tabs work only while there is a result to list"
        );
        assert!(
            !says(&harness, "Go to Ingest"),
            "{name} offers no way to the Ingest page"
        );
        testkit::save_png(&mut harness, &format!("answer-{name}"));
    }
}

fn is_writing(harness: &Harness<'_, Host>) -> bool {
    harness
        .query_by_role_and_label(Role::ProgressIndicator, "Writing the answer")
        .is_some()
}

#[test]
fn results_are_listed_under_the_answer_tab_until_the_answer_is_written() {
    let mut harness = pane(TALL, testkit::searched(found()));
    harness.run();
    assert!(is_writing(&harness), "the answer is on its way");
    assert_eq!(
        shown_results(&harness),
        [1, 2, 3, 4, 5],
        "the results are there at once"
    );
    click_surface(&mut harness, "Result 2");
    assert_eq!(
        selected_after(&mut harness, 2),
        Some(2),
        "and they can be used"
    );

    deliver(&mut harness.state_mut().shared, |request| Event::Answer {
        request,
        result: Ok(written()),
    });
    harness.run();
    assert!(!is_writing(&harness), "the answer has landed");
    assert!(says(&harness, "How a call is priced"), "its title is there");
    assert_eq!(
        shown_results(&harness),
        [1, 4, 5],
        "the rows gave way to the cards"
    );

    let mut harness = pane(TALL, results_only());
    harness.run();
    assert!(!is_writing(&harness), "no answer was asked for");
    assert_eq!(
        shown_results(&harness),
        [1, 2, 3, 4, 5],
        "the Results tab is open"
    );
}

#[test]
fn a_new_ask_starts_the_pane_again() {
    let mut harness = pane(TALL, answered());
    harness.run();
    click_first(&mut harness, "Copy LaTeX");
    harness.state_mut().apply_intents();
    // Not a click: the pointer stays on the button, which keeps it reading Copied.
    harness
        .get_by_role_and_label(Role::Tab, "Results")
        .click_accesskit();
    harness.run();
    assert_eq!(
        shown_results(&harness),
        [1, 2, 3, 4, 5],
        "the Results tab is open"
    );

    let shared = &mut harness.state_mut().shared;
    let draft = AskDraft {
        question: "What is a put?".to_owned(),
        ..AskDraft::default()
    };
    apply(shared, Intent::Ask(draft));
    deliver(shared, |request| Event::Search {
        request,
        result: Ok(found()),
    });
    deliver(shared, |request| Event::Answer {
        request,
        result: Ok(written()),
    });
    harness.run();
    assert!(
        says(&harness, "How a call is priced"),
        "the Answer tab is open again"
    );
    let copied = harness
        .query_all_by_role_and_label(Role::Button, "Copied")
        .count();
    assert_eq!(copied, 0, "no button keeps the mark of the old ask");
    harness.get_by_role_and_label(Role::Button, "Copy LaTeX");
}
