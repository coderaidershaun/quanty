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
    AskDraft, Command, DocId, Event, Failure, FailureKind, Intent, Loadable, PageBox, PageView,
    PieceKind, RequestId, SearchReply, StartupFacts, Tab,
};
use gui::state::{HealthLevel, Shared};
use gui::testkit::{self, sample};
use uuid::Uuid;

const LIMIT: Duration = Duration::from_secs(5);

fn fake_of(scene: &str) -> Fake {
    Fake::scene(scene, &testkit::samples_folder())
        .unwrap_or_else(|error| panic!("scene `{scene}` does not start: {error}"))
        .instant()
}

fn doc(number: u128) -> DocId {
    DocId(Uuid::from_u128(number))
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

/// The events a handler sends for one command, in order.
fn serve_all(fake: &Fake, command: Command) -> Vec<Event> {
    let (reply, received, _stop) = Reply::collecting();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime starts")
        .block_on(fake.serve(command, reply));
    received.try_iter().collect()
}

/// The page and the concepts that the fake sends for one page, and nothing else.
fn load_page(fake: &Fake, doc: DocId, page: u32) -> (Result<PageView, Failure>, Event) {
    let command = Command::LoadPage {
        request: RequestId(1),
        doc,
        page,
        folder: None,
    };
    let mut events = serve_all(fake, command).into_iter();
    let (page, concepts) = (events.next(), events.next());
    assert!(events.next().is_none(), "a page load sends two events");
    match (page, concepts) {
        (Some(Event::Page { result, .. }), Some(concepts)) => (result, concepts),
        other => panic!("a page load did not send a page and its concepts: {other:?}"),
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
    let ok = match name {
        "idle" | "gallery" => documents == Some(3) && ask.search == Loadable::Idle,
        "first-run" | "empty-library" => documents == Some(0) && ask.search == Loadable::Idle,
        "black-scholes" => results == Some(9) && nodes > Some(0) && blocks > Some(0),
        "no-answer" => results == Some(9) && blocks == Some(0),
        "results-only" => results == Some(9) && nodes > Some(0) && ask.answer == Loadable::Idle,
        "searching" => ask.search.is_loading(),
        "answering" => results == Some(9) && ask.answer.is_loading(),
        "no-sources" => results == Some(0),
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

fn assert_page_loads(fake: &Fake, doc_number: u128, page: u32, pages: u32) -> Event {
    let (view, concepts) = load_page(fake, doc(doc_number), page);
    let view = view.unwrap_or_else(|failure| panic!("page {page} of {doc_number}: {failure:?}"));
    assert!(matches!(
        concepts,
        Event::PageConcepts { result: Ok(_), .. }
    ));
    assert_eq!(
        (view.doc, view.page, view.page_count),
        (doc(doc_number), page, pages)
    );
    assert!(view.book.is_some() && view.chapter.is_some() && !view.pieces.is_empty());
    for piece in &view.pieces {
        assert!(
            !piece.text.ends_with('\n'),
            "a piece keeps no closing newline"
        );
        let picture = piece.image.as_ref().map(|image| image.path.is_file());
        assert_eq!(
            picture.is_some(),
            piece.kind == PieceKind::Figure,
            "only a figure has a picture"
        );
        assert!(picture.unwrap_or(true), "a picture that is named is there");
    }
    let has_pictures = doc_number == 3;
    assert_eq!(
        view.image.is_some(),
        has_pictures,
        "only the book has page pictures"
    );
    assert_eq!(view.previous_image.is_some(), has_pictures && page > 1);
    assert_eq!(view.next_image.is_some(), has_pictures && page < pages);
    concepts
}

fn assert_known_pages(fake: &Fake) {
    let figure_page = load_page(fake, doc(3), 5)
        .0
        .expect("page 5 of the book loads");
    assert_eq!(figure_page.printed_page.as_deref(), Some("233"));
    assert_eq!(figure_page.pieces.len(), 4);
    let figure = &figure_page.pieces[0];
    assert_eq!(figure.label.as_deref(), Some("Figure 13-4"));
    let cut = PageBox {
        left: 15,
        top: 23,
        right: 905,
        bottom: 485,
    };
    assert_eq!(figure.cut, Some(cut));
    let picture = figure.image.as_ref().expect("the figure has its picture");
    assert!(picture.path.ends_with("page-num-5/01-figure.png"));

    let notes_page = load_page(fake, doc(2), 3)
        .0
        .expect("page 3 of chapter 2 loads");
    assert_eq!(notes_page.printed_page.as_deref(), Some("7"));
    assert_eq!(notes_page.pieces.len(), 10);
    assert_eq!(notes_page.pieces[2].kind, PieceKind::Heading { rank: 2 });
    let formula = &notes_page.pieces[4];
    assert_eq!(formula.kind, PieceKind::Formula);
    assert_eq!(formula.label.as_deref(), Some("(2.4)"));
    assert_eq!(formula.name.as_deref(), Some("Black–Scholes call price"));

    let missing = load_page(fake, doc(3), 8)
        .0
        .expect_err("page 8 of 7 does not load");
    assert_eq!(missing.kind, FailureKind::SourceMissing);
}

#[test]
fn every_scene_opens_and_every_sample_page_loads() {
    assert_eq!(fake::scenes().len(), 17);
    for scene in fake::scenes() {
        let mut harness = open_scene(scene.name, scene.rests);
        assert_scene(scene.name, harness.state().shared());
        testkit::save_png(&mut harness, &format!("scene-{}", scene.name));
    }

    let fake = fake_of("idle");
    let mut empty_pages = 0;
    for (doc_number, pages) in [(1, 3), (2, 3), (3, 7)] {
        for page in 1..=pages {
            let concepts = assert_page_loads(&fake, doc_number, page, pages);
            empty_pages += usize::from(
                matches!(concepts, Event::PageConcepts { result: Ok(list), .. } if list.is_empty()),
            );
        }
    }
    assert!(
        empty_pages > 0 && empty_pages < 13,
        "some sample pages have concepts and some have none"
    );
    assert_known_pages(&fake);
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
            // Every other command, such as the catalogue load at start-up, is answered with a
            // failure, so only the ask is left waiting.
            for event in command.failed(&Failure::internal("the stand-in does not serve this")) {
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

    let mut harness = app_on(PanicsAfterTheResults, asks());
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
    press(&mut harness, command, egui::Key::K);
    assert_eq!(harness.state().shared().cues.focus_ask_bar, 1);
    press(&mut harness, none, egui::Key::Slash);
    assert_eq!(harness.state().shared().cues.focus_ask_bar, 2);
    harness.state_mut().push(Intent::OpenTab(Tab::Library));
    harness.run_ok();
    assert_eq!(harness.state().shared().tab, Tab::Library);
    press(&mut harness, command, egui::Key::Num1);
    assert_eq!(harness.state().shared().tab, Tab::Ask);

    // A row of a part that is not built does nothing and sends nothing.
    testkit::settle(&mut harness);
    for key in [
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::R,
        egui::Key::Slash,
    ] {
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
