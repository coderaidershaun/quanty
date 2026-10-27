//! The footer shows what is known of the Google and the Jev account on every tab, reads it only
//! at start, after a paid job and on a click, and a read that fails or never ends blocks nothing.

use std::time::{Duration, Instant};

use backend::fake::Fake;
use backend::{Handler, Reply};
use contract::{
    AskDraft, Balance, BalanceReads, Category, Command, DatasetRequest, DocumentName, Effect,
    Event, Failure, FailureKind, IngestRequest, Intent, Provider, RequestId, RetrievalTrace,
    SearchReply, Tab,
};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Node;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::{DEFAULT_WINDOW, PHONE_WINDOW};
use gui::state::Shared;
use gui::testkit::{self, run_until};

use super::fit;
use super::recording::{self, Seen};
use super::{Window, click, click_in, is_enabled, node_in, open_tab, panels, shared};

const GOOGLE_PAGE: &str = "https://aistudio.google.com/billing";
const JEV_PAGE: &str = "https://console.typesafe.ai/";
const LIMIT: Duration = Duration::from_secs(5);

/// The nodes of that role whose name holds `words`, in the footer of a window of that size.
fn in_footer<'a>(
    harness: &'a Window,
    role: Role,
    words: &'a str,
    window: [f32; 2],
) -> Vec<Node<'a>> {
    let footer = panels(window).footer;
    harness
        .query_all_by_label_contains(words)
        .filter(|node| {
            node.accesskit_node().role() == role && footer.contains(node.rect().center())
        })
        .collect()
}

fn says_in_footer(harness: &Window, words: &str, window: [f32; 2]) -> bool {
    let footer = panels(window).footer;
    harness
        .query_all_by_label_contains(words)
        .any(|node| footer.contains(node.rect().center()))
}

const LINKS: [(&str, &str); 2] = [
    ("Open the Google billing page", GOOGLE_PAGE),
    ("Open Jev billing in the console", JEV_PAGE),
];

/// One entry for each account and its link, each entry saying `state`.
fn assert_both_entries_say(harness: &Window, state: &str, scene: &str) {
    for (entry, (link, _)) in ["Google Gemini:", "Jev:"].into_iter().zip(LINKS) {
        let entries = in_footer(harness, Role::Button, entry, DEFAULT_WINDOW);
        assert_eq!(
            entries.len(),
            1,
            "`{scene}` has {} `{entry}`",
            entries.len()
        );
        let name = entries[0].accesskit_node().label().unwrap_or_default();
        assert!(name.contains(state), "`{scene}`: `{name}` is not `{state}`");
        let footer = panels(DEFAULT_WINDOW).footer;
        assert!(
            node_in(harness, Role::Button, link, footer).is_some(),
            "`{scene}` has no `{link}`"
        );
    }
}

/// A key that was taken leaves its account a dot and a billing link, with no words of state.
fn assert_both_entries_are_links(harness: &Window, scene: &str) {
    let footer = panels(DEFAULT_WINDOW).footer;
    for (entry, (link, _)) in ["Google Gemini:", "Jev:"].into_iter().zip(LINKS) {
        let entries = in_footer(harness, Role::Button, entry, DEFAULT_WINDOW);
        assert!(entries.is_empty(), "`{scene}` has a `{entry}` entry");
        assert!(
            node_in(harness, Role::Button, link, footer).is_some(),
            "`{scene}` has no `{link}`"
        );
    }
    assert!(
        !says_in_footer(harness, "balance not offered", DEFAULT_WINDOW),
        "`{scene}` says the balance is not offered"
    );
}

fn reads(seen: &Seen) -> usize {
    seen.count(|command| matches!(command, Command::ReadBalances { .. }))
}

fn question() -> Intent {
    Intent::Ask(AskDraft {
        question: "How is the Black–Scholes formula derived?".to_owned(),
        ..Default::default()
    })
}

fn every_tab_has_one_entry_for_each_account() {
    let (mut harness, _) = recording::open("idle", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    for tab in Tab::ALL {
        open_tab(&mut harness, tab);
        assert_both_entries_are_links(&harness, &format!("{tab:?}"));
    }
    open_tab(&mut harness, Tab::Ask);
    let graph = panels(DEFAULT_WINDOW).concept_graph;
    node_in(&harness, Role::Button, "Maximise", graph)
        .expect("the Concept Graph has a Maximise button")
        .click();
    testkit::settle(&mut harness);
    assert_both_entries_are_links(&harness, "a maximised graph");
}

fn opened(harness: &Window) -> Vec<String> {
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
            _ => None,
        })
        .collect()
}

/// The address a click opens is in the output of that one frame, so it is read before the app
/// settles.
fn opened_by(harness: &mut Window, act: impl FnOnce(&mut Window)) -> Vec<String> {
    act(harness);
    harness.step();
    let urls = opened(harness);
    testkit::settle(harness);
    urls
}

fn each_link_opens_the_page_of_its_provider() {
    let (mut harness, _) = recording::open("idle", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    for (link, page) in LINKS {
        let footer = panels(DEFAULT_WINDOW).footer;
        let urls = opened_by(&mut harness, |harness| {
            node_in(harness, Role::Button, link, footer)
                .unwrap_or_else(|| panic!("no `{link}` in the footer"))
                .click();
        });
        assert_eq!(urls, [page], "{link}");
    }
}

/// On a phone each account is a dot and a short name, the connection stands beside them, and none
/// of the three overlaps another. A tap on an account opens the full lines with the billing links.
fn a_phone_footer_is_one_row_that_opens_the_full_lines() {
    let (mut harness, _) = recording::open("idle", PHONE_WINDOW);
    testkit::settle(&mut harness);
    let footer = panels(PHONE_WINDOW).footer;
    let screen = fit::window_rect(PHONE_WINDOW);
    for tab in Tab::ALL {
        open_tab(&mut harness, tab);
        let mut parts = Vec::new();
        for entry in ["Google Gemini", "Jev"] {
            let found = in_footer(&harness, Role::Button, entry, PHONE_WINDOW);
            assert_eq!(found.len(), 1, "{tab:?}: {} `{entry}`", found.len());
            let name = found[0].accesskit_node().label().unwrap_or_default();
            assert_eq!(
                name, entry,
                "{tab:?}: a key that was taken has no words of state"
            );
            parts.push((format!("{tab:?}: {entry}"), found[0].rect()));
        }
        let connection = in_footer(&harness, Role::Label, "Connected", PHONE_WINDOW);
        assert_eq!(connection.len(), 1, "{tab:?}: the connection is not shown");
        parts.push((format!("{tab:?}: Connected"), connection[0].rect()));
        fit::assert_inside_and_apart(&parts, footer, screen);
    }

    click_in(&mut harness, Role::Button, "Google Gemini", footer);
    testkit::save_png(&mut harness, "phone-footer-open");
    for (link, page) in LINKS {
        let urls = opened_by(&mut harness, |harness| {
            node_in(harness, Role::Button, link, screen)
                .unwrap_or_else(|| panic!("no `{link}` is on screen"))
                .click();
        });
        assert_eq!(urls, [page], "{link}");
    }
}

fn start_a_build(harness: &mut Window) {
    let build = DatasetRequest {
        name: "notes-v2".to_owned(),
        media: None,
        holdout: Vec::new(),
        rewrites: 4,
        questions: 2,
        max_sft: 300,
        dry_run: false,
    };
    harness.state_mut().push(Intent::StartDatasetBuild(build));
    testkit::settle(harness);
}

fn balances_are_read_at_start_after_a_paid_job_and_on_a_click_only() {
    // This scene opens with the check of a chapter, which is free and reads nothing.
    let (mut harness, seen) = recording::open("ingest-ready", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert_eq!(reads(&seen), 1, "one read at start, and none from a check");
    click(&mut harness, Role::Button, "Start ingest");
    assert_eq!(reads(&seen), 2, "one read after the ingest ended");

    harness.state_mut().push(question());
    testkit::settle(&mut harness);
    assert_eq!(reads(&seen), 3, "one read after the ask ended");

    start_a_build(&mut harness);
    assert_eq!(reads(&seen), 4, "one read after the build ended");

    for _ in 0..50 {
        harness.step();
    }
    std::thread::sleep(Duration::from_millis(50));
    testkit::settle(&mut harness);
    assert_eq!(reads(&seen), 4, "an app at rest reads nothing");

    // A key that was taken is a link alone, so the click is on an entry that has a state.
    let (mut harness, seen) = recording::open("balances-unread", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert_eq!(reads(&seen), 1, "one read at start");
    let footer = panels(DEFAULT_WINDOW).footer;
    click_in(
        &mut harness,
        Role::Button,
        "Google Gemini: could not read",
        footer,
    );
    assert_eq!(reads(&seen), 2, "one read on a click");

    let (mut harness, seen) = recording::open("searching", DEFAULT_WINDOW);
    // The start read must end first, so that it is counted before the ask is stopped.
    run_until(&mut harness, "the ask to start", |shared| {
        shared.ask.is_running() && !shared.balances.is_reading()
    });
    assert_eq!(reads(&seen), 1, "an ask that is running has not ended");
    harness.key_press(egui::Key::Escape);
    testkit::settle(&mut harness);
    assert_eq!(reads(&seen), 2, "one read after a cancelled ask");
}

fn a_key_that_is_missing_names_its_setting() {
    let (mut harness, _) = recording::open("first-run", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert_both_entries_say(&harness, "key missing", "first-run");
    for words in [
        "EMBEDDING_GEMINI_API_KEY",
        "CONVERTER_JEV_API_KEY",
        "start quanty again",
    ] {
        assert!(
            says_in_footer(&harness, words, DEFAULT_WINDOW),
            "the footer lacks `{words}`"
        );
    }
}

fn a_read_that_failed_says_so_and_blocks_nothing() {
    let (mut harness, _) = recording::open("balances-unread", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let google = in_footer(&harness, Role::Button, "Google Gemini:", DEFAULT_WINDOW);
    let name = google[0].accesskit_node().label().unwrap_or_default();
    assert!(name.contains("could not read"), "{name}");
    assert!(says_in_footer(
        &harness,
        "Click to try again",
        DEFAULT_WINDOW
    ));
    let jev = in_footer(&harness, Role::Button, "Jev:", DEFAULT_WINDOW);
    let name = jev[0].accesskit_node().label().unwrap_or_default();
    assert!(name.contains("key refused"), "{name}");
    assert!(says_in_footer(
        &harness,
        "start quanty again",
        DEFAULT_WINDOW
    ));

    harness.state_mut().push(question());
    testkit::settle(&mut harness);
    let shared = shared(&harness);
    assert!(shared.ask.answer.ready().is_some(), "the answer came");
    assert!(
        shared.library.catalogue.ready().is_some(),
        "the library loaded"
    );
}

/// Answers every command but a read of the balances, which hangs for ever.
struct ReadsNeverEnd(Fake);

impl Handler for ReadsNeverEnd {
    async fn serve(&self, command: Command, reply: Reply) {
        if matches!(command, Command::ReadBalances { .. }) {
            return std::future::pending().await;
        }
        self.0.serve(command, reply).await;
    }
}

fn a_read_that_never_ends_blocks_nothing() {
    let (mut harness, _) = recording::open_with("idle", DEFAULT_WINDOW, ReadsNeverEnd);
    harness.state_mut().push(question());
    // The read never ends, so the app is never idle, and `settle` would wait for ever.
    run_until(&mut harness, "the answer", |shared| {
        shared.ask.answer.ready().is_some()
    });
    assert!(shared(&harness).library.catalogue.ready().is_some());
    assert_both_entries_say(&harness, "checking…", "a read that never ends");
    let started = Instant::now();
    while !is_enabled(&harness, Role::ComboBox, "Model") {
        assert!(started.elapsed() < LIMIT, "the model list stayed off");
        harness.run_ok();
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn spent_credit_is_shown_until_a_job_that_needs_the_provider_works() {
    let (mut harness, _) = recording::open("out-of-credit", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    let google = in_footer(&harness, Role::Button, "Google Gemini:", DEFAULT_WINDOW);
    let name = google[0].accesskit_node().label().unwrap_or_default();
    assert!(name.contains("out of credit"), "{name}");
    assert!(says_in_footer(&harness, "clears this", DEFAULT_WINDOW));
    let jev = in_footer(&harness, Role::Button, "Jev:", DEFAULT_WINDOW);
    assert!(
        jev.is_empty(),
        "a Jev key that was taken has no entry of its own"
    );
    let hint = FailureKind::EmbeddingOutOfCredit.hint();
    assert!(harness.query_all_by_label_contains(hint).next().is_some());

    harness.state_mut().push(Intent::OpenTab(Tab::Ingest));
    harness.state_mut().push(Intent::CheckIngest(IngestRequest {
        pdf: contract::UploadId::default(),
        file_name: "chapter-3-greeks.pdf".into(),
        media: "Option Volatility and Pricing".to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 3,
            name: "Greeks".to_owned(),
        },
        tags: Vec::new(),
    }));
    testkit::settle(&mut harness);
    harness.state_mut().push(Intent::StartIngest);
    testkit::settle(&mut harness);
    let jev = in_footer(&harness, Role::Button, "Jev:", DEFAULT_WINDOW);
    let name = jev[0].accesskit_node().label().unwrap_or_default();
    assert!(name.contains("out of credit"), "{name}");
    let hint = FailureKind::ConverterOutOfCredit.hint();
    assert!(harness.query_all_by_label_contains(hint).next().is_some());
}

// SMELL: this file is over 400 lines. The checks from here down drive the shared state alone, with
// no window, and are the part to move out first.

/// The request that the shared state sent for the read it was asked for.
fn read_request(effects: &[Effect]) -> RequestId {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::Send(Command::ReadBalances { request }) => Some(*request),
            _ => None,
        })
        .expect("a read was sent")
}

fn read_found(shared: &mut Shared, found: BalanceReads) {
    let mut effects = Vec::new();
    shared.apply_intent(Intent::ReadBalances, &mut effects);
    let request = read_request(&effects);
    shared.apply_event(Event::Balances { request, found }, &mut Vec::new());
}

fn search_ended(shared: &mut Shared, result: Result<SearchReply, Failure>) {
    // The note listens to the event itself, so any request id will do.
    let event = Event::Search {
        request: RequestId(99),
        result,
    };
    shared.apply_event(event, &mut Vec::new());
}

fn shows(shared: &Shared) -> (Option<Balance>, Option<Balance>) {
    (
        shared.balances.shown(Provider::Google),
        shared.balances.shown(Provider::Jev),
    )
}

fn the_note_of_spent_credit_comes_from_a_reply_and_goes_with_a_reply_that_worked() {
    let not_offered = BalanceReads {
        google: Balance::NotOffered,
        jev: Balance::NotOffered,
    };
    let mut state = Shared::default();
    assert_eq!(shows(&state), (None, None), "before the first read ends");
    read_found(&mut state, not_offered);
    assert_eq!(
        shows(&state),
        (Some(Balance::NotOffered), Some(Balance::NotOffered))
    );

    let spent = Failure::new(FailureKind::EmbeddingOutOfCredit, "Google said so");
    search_ended(&mut state, Err(spent.clone()));
    assert_eq!(
        shows(&state),
        (Some(Balance::OutOfCredit), Some(Balance::NotOffered))
    );

    // A search with no document to look at never asked Google, so the note stays.
    let searched_nothing = SearchReply {
        trace: RetrievalTrace {
            documents_searched: Some(0),
            ..RetrievalTrace::default()
        },
        ..SearchReply::default()
    };
    search_ended(&mut state, Ok(searched_nothing));
    assert_eq!(shows(&state).0, Some(Balance::OutOfCredit));

    search_ended(&mut state, Ok(SearchReply::default()));
    assert_eq!(shows(&state).0, Some(Balance::NotOffered));

    // A key that is refused matters more than any sighting of credit.
    read_found(
        &mut state,
        BalanceReads {
            google: Balance::KeyRefused,
            jev: Balance::NotOffered,
        },
    );
    search_ended(&mut state, Err(spent.clone()));
    assert_eq!(shows(&state).0, Some(Balance::KeyRefused));

    // So does an address that Google refuses: no credit is seen until a call gets through.
    read_found(
        &mut state,
        BalanceReads {
            google: Balance::LocationRefused,
            jev: Balance::NotOffered,
        },
    );
    search_ended(&mut state, Err(spent));
    assert_eq!(shows(&state).0, Some(Balance::LocationRefused));
}

#[test]
fn the_footer_shows_each_account_and_reads_it_only_when_asked() {
    every_tab_has_one_entry_for_each_account();
    each_link_opens_the_page_of_its_provider();
    a_phone_footer_is_one_row_that_opens_the_full_lines();
    balances_are_read_at_start_after_a_paid_job_and_on_a_click_only();
    a_key_that_is_missing_names_its_setting();
    a_read_that_failed_says_so_and_blocks_nothing();
    a_read_that_never_ends_blocks_nothing();
    spent_credit_is_shown_until_a_job_that_needs_the_provider_works();
    the_note_of_spent_credit_comes_from_a_reply_and_goes_with_a_reply_that_worked();
}
