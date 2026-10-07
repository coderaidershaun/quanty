//! Checks that the runtime hands each command to a backend, gives back its events in order,
//! and stops the work when a command is cancelled.

use std::time::{Duration, Instant};

use eframe::egui;
use egui_kittest::Harness;
use gui::app::App;
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::{self, Fake};
use gui::backend::{Backend, Handler, Reply};
use gui::contract::{
    AskDraft, Command, Event, Failure, FailureKind, Intent, Loadable, RequestId, SearchReply,
    StartupFacts, Tab,
};
use gui::state::{HealthLevel, IngestJob, Shared};
use gui::testkit::{self, sample};

use super::sample_pages;

const LIMIT: Duration = Duration::from_secs(5);

fn fake_of(scene: &str) -> Fake {
    Fake::scene(scene, &testkit::samples_folder())
        .unwrap_or_else(|error| panic!("scene `{scene}` does not start: {error}"))
        .instant()
}

/// Runs the frames of the app until `done` holds for its shared state.
fn run_until(harness: &mut Harness<'_, App>, what: &str, done: impl Fn(&Shared) -> bool) {
    let started = Instant::now();
    while !done(harness.state().shared()) {
        assert!(started.elapsed() < LIMIT, "gave up waiting for {what}");
        harness.run_ok();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The next event of the runtime, waited for.
fn next_event(backend: &mut Backend) -> Event {
    let started = Instant::now();
    loop {
        if let Some(event) = backend.try_next() {
            return event;
        }
        assert!(started.elapsed() < LIMIT, "the next event never came");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn is_failed<T>(slot: &Loadable<T>, kind: FailureKind) -> bool {
    slot.failure().is_some_and(|failure| failure.kind == kind)
}

/// What the scene must have put in the shared state once it has opened and settled.
fn assert_scene(name: &str, shared: &Shared) {
    let (ask, source) = (&shared.ask, &shared.source);
    let documents = shared
        .library
        .catalogue
        .ready()
        .map(|found| found.documents().count());
    let results = ask.search.ready().map(|reply| reply.results.len());
    let nodes = ask.graph.ready().map(|graph| graph.nodes.len());
    let blocks = ask.answer.ready().map(|answer| answer.blocks.len());
    let trace = ask.search.ready().map(|reply| &reply.trace);
    // With no seed the search stops at its first step, so no later step has a value.
    let stopped_at_the_first_step = trace.is_some_and(|trace| *trace == Default::default());
    let ok = match name {
        "idle" | "gallery" => documents == Some(3) && ask.search == Loadable::Idle,
        "first-run" | "empty-library" => documents == Some(0) && ask.search == Loadable::Idle,
        "black-scholes" => results == Some(9) && nodes > Some(0) && blocks > Some(0),
        "no-answer" => results == Some(9) && blocks == Some(0),
        "results-only" => results == Some(9) && nodes > Some(0) && ask.answer == Loadable::Idle,
        "searching" => ask.search.is_loading(),
        "answering" => results == Some(9) && ask.answer.is_loading(),
        "no-sources" => results == Some(0) && stopped_at_the_first_step,
        "search-failed" => is_failed(&ask.search, FailureKind::EmbeddingFailed),
        "answer-failed" => {
            results == Some(9) && is_failed(&ask.answer, FailureKind::ClaudeUsageLimit)
        }
        "graph-failed" => is_failed(&ask.graph, FailureKind::FalkorDbDown) && blocks > Some(0),
        "no-labels" => {
            results == Some(0) && trace.is_some_and(|trace| trace.documents_searched == Some(0))
        }
        "no-concepts" => {
            let no_seeds = trace.is_some_and(|trace| trace.seed_concepts == Some(Vec::new()));
            results == Some(9) && nodes == Some(0) && blocks > Some(0) && no_seeds
        }
        "source-missing" => {
            let is_empty = source.concepts == Loadable::Ready(Vec::new());
            results == Some(9) && is_failed(&source.page, FailureKind::SourceMissing) && is_empty
        }
        "stores-down" => {
            is_failed(&ask.search, FailureKind::QdrantDown)
                && is_failed(&shared.library.catalogue, FailureKind::QdrantDown)
                && source.page.ready().is_some()
                && is_failed(&source.concepts, FailureKind::FalkorDbDown)
        }
        "ingest-ready" | "ingest-failed" => {
            let is_checked = matches!(
                &shared.ingest,
                IngestJob::Checked { preflight, .. }
                    if preflight.chapter.number == 3 && preflight.blockers.is_empty()
            );
            shared.tab == Tab::Ingest && is_checked
        }
        other => panic!("the test has no expectation for the scene `{other}`"),
    };
    assert!(ok, "scene `{name}` opened in the wrong state: {shared:#?}");
}

fn open_scene(name: &str, rests: bool) -> Harness<'static, App> {
    let mut harness = testkit::app(name, DEFAULT_WINDOW);
    if rests {
        testkit::settle(&mut harness);
        return harness;
    }
    // A scene that never rests is looked at once the state it shows has arrived.
    let (what, arrived): (&str, fn(&Shared) -> bool) = match name {
        "answering" => ("the results", |shared| shared.ask.search.ready().is_some()),
        "gallery" => ("the catalogue", |shared| {
            shared.library.catalogue.ready().is_some()
        }),
        _ => ("the ask to start", |shared| shared.ask.is_running()),
    };
    run_until(&mut harness, what, arrived);
    harness
}

#[test]
fn every_scene_opens_and_every_sample_page_loads() {
    assert_eq!(fake::scenes().len(), 19);
    for scene in fake::scenes() {
        let mut harness = open_scene(scene.name, scene.rests);
        assert_scene(scene.name, harness.state().shared());
        testkit::save_png(&mut harness, &format!("scene-{}", scene.name));
    }

    sample_pages::assert_every_sample_page_loads(&fake_of("idle"));
}

#[test]
fn an_ask_shows_results_then_the_answer_and_cancel_stops_it() {
    let ask = |request| Command::Ask {
        request,
        ask: AskDraft {
            question: "How is the Black–Scholes formula derived?".to_owned(),
            ..AskDraft::default()
        },
    };
    let mut backend = Backend::start(fake_of("black-scholes"), || {}).expect("the runtime starts");
    backend.send(ask(RequestId(1)));
    let found = next_event(&mut backend);
    assert!(
        matches!(&found, Event::Search { result: Ok(reply), .. } if reply.results.len() == 9),
        "{found:?}"
    );
    let drawn = next_event(&mut backend);
    assert!(
        matches!(drawn, Event::Graph { result: Ok(_), .. }),
        "{drawn:?}"
    );
    let written = next_event(&mut backend);
    assert!(
        matches!(written, Event::Answer { result: Ok(_), .. }),
        "{written:?}"
    );

    let mut backend = Backend::start(fake_of("searching"), || {}).expect("the runtime starts");
    backend.send(ask(RequestId(2)));
    assert!(
        !backend.is_idle(),
        "a search that never returns keeps its task"
    );
    backend.send(Command::Cancel(RequestId(2)));
    let started = Instant::now();
    while !backend.is_idle() {
        assert!(started.elapsed() < LIMIT, "the cancel did not end the task");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        backend.try_next().is_none(),
        "a cancelled ask sends nothing"
    );

    let mut harness = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    assert!(harness.state().shared().ask.answer.ready().is_some());
    testkit::save_png(&mut harness, "shell-black-scholes");
}

/// Panics at once, whatever it is asked.
struct PanicsAtOnce;

impl Handler for PanicsAtOnce {
    async fn serve(&self, _command: Command, _reply: Reply) {
        panic!("a bug in an adapter");
    }
}

/// Sends the first results of an ask, and then panics on the way to the graph and the answer.
struct PanicsAfterTheResults;

impl Handler for PanicsAfterTheResults {
    async fn serve(&self, command: Command, reply: Reply) {
        let Command::Ask { request, .. } = command else {
            // Every other command, such as the catalogue load at start-up, is told that the
            // store is down, so only the ask is left waiting.
            let down = Failure::new(FailureKind::QdrantDown, "the stand-in has no store");
            for event in command.failed(&down) {
                reply.send(event);
            }
            return;
        };
        let one = sample::search_reply().results.remove(0);
        reply.send(Event::Search {
            request,
            result: Ok(SearchReply {
                results: vec![one],
                ..SearchReply::default()
            }),
        });
        panic!("a bug in an adapter, after the search");
    }
}

fn app_on(handler: impl Handler, opening: Vec<Intent>) -> Harness<'static, App> {
    let facts = StartupFacts {
        home: testkit::samples_folder(),
        ..StartupFacts::default()
    };
    Harness::builder()
        .with_size(egui::Vec2::from(DEFAULT_WINDOW))
        .build_eframe(move |creation| {
            App::new(creation, handler, facts, opening).expect("the backend's threads start")
        })
}

fn asks() -> Vec<Intent> {
    vec![Intent::Ask(AskDraft {
        question: "How is the Black–Scholes formula derived?".to_owned(),
        ..AskDraft::default()
    })]
}

fn is_a_bug(slot: &Loadable<impl Sized>) -> bool {
    slot.failure()
        .is_some_and(|failure: &Failure| failure.kind == FailureKind::Internal)
}

#[test]
fn a_backend_that_panics_gives_a_failure_and_never_an_endless_wait() {
    let mut opening = asks();
    opening.push(Intent::RecheckHealth);
    let mut harness = app_on(PanicsAtOnce, opening);
    testkit::settle(&mut harness);
    let shared = harness.state().shared();
    assert!(is_a_bug(&shared.ask.search), "the search says it failed");
    assert!(!shared.ask.is_running());
    assert!(is_a_bug(&shared.library.catalogue), "so does the catalogue");
    let health = &shared.health;
    assert!(health.pending.is_none(), "the check is over");
    assert_eq!(health.level(), HealthLevel::Unknown, "no service is down");

    // The catalogue load said the store is down, so the first results ask for a new check of
    // the services. Nobody clicks then: it must still be sent, or the app never comes to rest.
    let mut harness = app_on(PanicsAfterTheResults, Vec::new());
    testkit::settle(&mut harness);
    harness.state_mut().push(asks().remove(0));
    testkit::settle(&mut harness);
    let ask = &harness.state().shared().ask;
    assert_eq!(ask.search.ready().map(|reply| reply.results.len()), Some(1));
    assert!(is_a_bug(&ask.graph), "the graph says it failed");
    assert!(is_a_bug(&ask.answer), "the answer says it failed");
    assert!(!ask.is_running());
}

fn press(harness: &mut Harness<'_, App>, modifiers: egui::Modifiers, key: egui::Key) {
    harness.key_press_modifiers(modifiers, key);
    harness.step();
    harness.run_ok();
}

#[test]
fn command_shortcuts_reach_the_reducer() {
    let command = egui::Modifiers::COMMAND;
    let none = egui::Modifiers::NONE;

    // The keys that raise the Ask bar's cue, and the key that turns a tab.
    let mut harness = testkit::app("idle", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    // The plain key goes first: once ⌘K has put the caret in the question box, `/` is text.
    press(&mut harness, none, egui::Key::Slash);
    assert_eq!(harness.state().shared().cues.focus_ask_bar, 1);
    press(&mut harness, command, egui::Key::K);
    assert_eq!(harness.state().shared().cues.focus_ask_bar, 2);
    harness.state_mut().push(Intent::OpenTab(Tab::Library));
    harness.run_ok();
    assert_eq!(harness.state().shared().tab, Tab::Library);
    press(&mut harness, command, egui::Key::Num1);
    assert_eq!(harness.state().shared().tab, Tab::Ask);

    // ⌘3 opens the Ingest page and ⌘1 goes back to Ask.
    press(&mut harness, command, egui::Key::Num3);
    assert_eq!(harness.state().shared().tab, Tab::Ingest);
    press(&mut harness, command, egui::Key::Num1);
    assert_eq!(harness.state().shared().tab, Tab::Ask);

    // A row of a part that is not built does nothing and sends nothing.
    testkit::settle(&mut harness);
    for key in [egui::Key::Num2, egui::Key::R, egui::Key::Slash] {
        press(&mut harness, command, key);
        let shared = harness.state().shared();
        assert_eq!(
            shared.tab,
            Tab::Ask,
            "{key:?} opened a tab that is not built"
        );
        assert!(!shared.help_open, "{key:?} opened the help sheet");
        assert!(
            shared.health.pending.is_none(),
            "{key:?} started a health check"
        );
        assert!(harness.state().is_idle(), "{key:?} sent a command");
    }

    // The two keys that stop a running ask.
    let mut harness = testkit::app("searching", DEFAULT_WINDOW);
    run_until(&mut harness, "the ask to start", |shared| {
        shared.ask.is_running()
    });
    press(&mut harness, command, egui::Key::Period);
    assert!(
        !harness.state().shared().ask.is_running(),
        "⌘. stops the ask"
    );
    testkit::settle(&mut harness);
    harness.state_mut().push(asks().remove(0));
    run_until(&mut harness, "the second ask to start", |shared| {
        shared.ask.is_running()
    });
    press(&mut harness, none, egui::Key::Escape);
    assert!(
        !harness.state().shared().ask.is_running(),
        "Esc stops the ask"
    );
    testkit::settle(&mut harness);

    // The keys that turn the source's page, step through the results, and copy the answer.
    let mut harness = testkit::app("black-scholes", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    harness.state_mut().push(Intent::SelectResult(1));
    testkit::settle(&mut harness);
    let shown = |harness: &Harness<'_, App>| {
        harness
            .state()
            .shared()
            .source
            .target
            .map(|target| target.page)
    };
    assert_eq!(shown(&harness), Some(3));
    press(&mut harness, command, egui::Key::OpenBracket);
    assert_eq!(shown(&harness), Some(2), "⌘[ turns back a page");
    testkit::settle(&mut harness);
    press(&mut harness, command, egui::Key::CloseBracket);
    assert_eq!(shown(&harness), Some(3), "⌘] turns on a page");
    testkit::settle(&mut harness);
    press(&mut harness, none, egui::Key::J);
    assert_eq!(harness.state().shared().ask.selected_result, Some(2));
    press(&mut harness, none, egui::Key::K);
    assert_eq!(harness.state().shared().ask.selected_result, Some(1));
    testkit::settle(&mut harness);
    // One frame with the key, so that the frame's output is the one that holds the copy.
    harness.input_mut().events.push(egui::Event::Key {
        key: egui::Key::S,
        physical_key: Some(egui::Key::S),
        pressed: true,
        repeat: false,
        modifiers: command | egui::Modifiers::SHIFT,
    });
    harness.step();
    let copied = harness
        .output()
        .platform_output
        .commands
        .iter()
        .any(|output| matches!(output, egui::OutputCommand::CopyText(text) if text.starts_with("# Black–Scholes")));
    assert!(copied, "⇧⌘S copies the answer");
}
