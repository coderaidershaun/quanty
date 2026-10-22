//! The choices a person makes in the window: Ask, Ingest and Datasets each show and change their
//! own model in the top bar and Library shows none, and the database is chosen in a sheet before
//! anything loads and named at the right end of the footer.

use std::path::Path;

use choices::{ApiKey, DATABASES_FILE, Databases, LOCAL_DATABASE};
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable as _};
use gui::app::layout::{self, DEFAULT_WINDOW};
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Command, Event, Failure, Loadable, Service, ServiceState, StartupFacts, Tab};
use gui::testkit::{self, run_until};

use super::recording;
use super::{
    Window, click, has, is_enabled, node, node_in, panels, press, retype, shared, type_into,
};

#[test]
fn each_tab_shows_and_changes_its_own_model() {
    let mut harness = testkit::app("idle", DEFAULT_WINDOW);
    testkit::settle(&mut harness);

    click(&mut harness, Role::ComboBox, "Model");
    click(&mut harness, Role::Button, "Codex · GPT-6 Astra");
    click(&mut harness, Role::Tab, "Ingest");
    assert_eq!(
        node(&harness, Role::ComboBox, "Model").value().as_deref(),
        Some("Codex · GPT-6.1 Sol")
    );
    click(&mut harness, Role::ComboBox, "Model");
    click(&mut harness, Role::Button, "Codex · GPT-6 Luna");
    click(&mut harness, Role::Tab, "Datasets");
    assert_eq!(
        node(&harness, Role::ComboBox, "Model").value().as_deref(),
        Some("Codex · GPT-6.1 Sol")
    );
    click(&mut harness, Role::Tab, "Ask");
    assert_eq!(
        node(&harness, Role::ComboBox, "Model").value().as_deref(),
        Some("Codex · GPT-6 Astra")
    );
    click(&mut harness, Role::Tab, "Library");
    assert!(!has(&harness, Role::ComboBox, "Model"));
}

/// The window of a first start: no database is chosen, `home` is a folder of its own so that the
/// choice has a file to be saved in, and the fake answers as it does for every scene.
fn first_start_in(home: &Path) -> Window {
    let samples = testkit::samples_folder();
    let fake = Fake::scene("no-database", &samples)
        .expect("the scene `no-database` exists")
        .instant();
    let opening = fake.opening();
    let facts = StartupFacts {
        local_database: Fake::local_database(home),
        databases: fake.databases(),
        home: home.to_owned(),
        env_file: None,
        fixture: None,
        anthropic_api_key_set: false,
    };
    testkit::app_on(fake, facts, opening, DEFAULT_WINDOW)
}

/// egui sets the layer of a sheet at the start of the next frame, so two frames run first.
fn is_a_sheet_open(harness: &mut Window) -> bool {
    harness.step();
    harness.step();
    harness
        .ctx
        .memory(|memory| memory.top_modal_layer().is_some())
}

fn click_at(harness: &mut Window, position: egui::Pos2) {
    harness.hover_at(position);
    harness.drag_at(position);
    harness.drop_at(position);
    harness.run_ok();
}

/// The entry at the right end of the footer, which says which database is in use.
fn database_entry<'a>(harness: &'a Window, name: &'a str) -> Option<egui_kittest::Node<'a>> {
    let footer = layout::footer(panels(DEFAULT_WINDOW).footer);
    node_in(harness, Role::Button, name, footer.database)
}

/// Whether a node outside the footer says `words`, which only the sheet does.
fn the_sheet_says(harness: &Window, words: &str) -> bool {
    let footer = panels(DEFAULT_WINDOW).footer;
    harness
        .query_all_by_label_contains(words)
        .any(|node| !footer.contains(node.rect().center()))
}

fn the_first_start_asks_which_database_before_anything_loads(harness: &mut Window) {
    testkit::settle(harness);
    assert!(
        is_a_sheet_open(harness),
        "the sheet is not the first thing shown"
    );
    assert!(has(harness, Role::Button, "Use"));

    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(
        is_a_sheet_open(harness),
        "Escape closed the first-start sheet"
    );
    click_at(harness, egui::pos2(8.0, 300.0));
    assert!(
        is_a_sheet_open(harness),
        "a click outside closed the first-start sheet"
    );
    assert!(
        matches!(shared(harness).library.catalogue, Loadable::Idle),
        "the catalogue was asked for before a database was chosen"
    );
}

fn using_the_database_saves_the_choice_and_names_it_on_every_tab(
    harness: &mut Window,
    home: &Path,
) {
    click(harness, Role::Button, "Use");
    assert!(!is_a_sheet_open(harness), "Use left the sheet open");
    let saved = Databases::load_from(&home.join(DATABASES_FILE)).expect("the file can be read");
    assert_eq!(saved.chosen.as_deref(), Some(LOCAL_DATABASE));
    assert!(
        shared(harness).library.catalogue.ready().is_some(),
        "the catalogue did not load after Use"
    );
    for tab in Tab::ALL {
        harness
            .state_mut()
            .push(gui::contract::Intent::OpenTab(tab));
        testkit::settle(harness);
        assert!(
            database_entry(harness, "Database: Local · connected").is_some(),
            "the {tab:?} tab lacks the database entry"
        );
    }

    // The sheet opens again, and it is not shut by the choice that was saved before.
    database_entry(harness, "Database: Local · connected")
        .expect("the entry is there")
        .click();
    testkit::settle(harness);
    assert!(
        is_a_sheet_open(harness),
        "a click on the entry did not open the sheet"
    );
    click(harness, Role::Button, "Cancel");
    assert!(!is_a_sheet_open(harness), "Cancel left the sheet open");
}

fn a_database_that_is_down_says_so_in_the_footer_and_in_the_sheet() {
    let mut harness = testkit::app("stores-down", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    database_entry(&harness, "Database: Local · down")
        .expect("the entry says the database is down")
        .click();
    testkit::settle(&mut harness);
    let hint = shared(&harness)
        .database
        .state
        .failure()
        .expect("the check of the database failed")
        .hint
        .clone();
    assert!(is_a_sheet_open(&mut harness));
    assert!(the_sheet_says(&harness, &hint), "the sheet lacks `{hint}`");
}

fn no_switch_lands_under_a_running_ingest() {
    // The scene only checks the chapter at its opening. Its ingest runs once it is started.
    let mut harness = testkit::app("ingest-running", DEFAULT_WINDOW);
    testkit::settle(&mut harness);
    node(&harness, Role::Button, "Start ingest").click();
    run_until(&mut harness, "the ingest to run", |shared| {
        shared.ingest.is_running()
    });
    database_entry(&harness, "Database: Local · connected")
        .expect("the entry is there")
        .click();
    // The ingest never ends, so `settle` would wait for ever.
    harness.run_ok();
    assert!(is_a_sheet_open(&mut harness));
    assert!(
        !is_enabled(&harness, Role::Button, "Use"),
        "Use is on while an ingest runs"
    );
}

/// Whether any node of the window holds `text` in any of its properties.
fn a_node_holds(harness: &Window, text: &str) -> bool {
    harness
        .query_all(By::new().predicate(|node| format!("{:?}", node.data()).contains(text)))
        .next()
        .is_some()
}

fn a_key_typed_into_the_sheet_is_in_no_node_but_is_saved(home: &Path) {
    const KEY: &str = "qdrant-key-8f3a91c4";
    const NAME: &str = "Office server";
    let mut harness = first_start_in(home);
    testkit::settle(&mut harness);
    click(&mut harness, Role::ComboBox, "Database");
    click(&mut harness, Role::Button, "New…");

    type_into(&mut harness, "Name", NAME);
    node(&harness, Role::PasswordInput, "Qdrant API key").click();
    node(&harness, Role::PasswordInput, "Qdrant API key").type_text(KEY);
    harness.run_ok();
    assert!(
        a_node_holds(&harness, NAME),
        "a typed name is not found, so the search is blind"
    );
    assert!(
        !a_node_holds(&harness, KEY),
        "the key is in a node of the window"
    );

    retype(&mut harness, "Qdrant URL", "http://127.0.0.1:16334");
    type_into(&mut harness, "FalkorDB URL", "falkor://127.0.0.1:16379");
    let folder = home.to_string_lossy().into_owned();
    type_into(&mut harness, "Content folder", &folder);
    click(&mut harness, Role::Button, "Use");
    let saved = Databases::load_from(&home.join(DATABASES_FILE)).expect("the file can be read");
    assert_eq!(saved.chosen.as_deref(), Some(NAME));
    assert_eq!(
        saved
            .saved
            .first()
            .and_then(|saved| saved.qdrant_api_key.clone()),
        Some(ApiKey::new(KEY)),
        "the key that was typed did not reach the file"
    );
}

/// Answers a Use of a saved database with a Qdrant that is down, and everything else as the fake
/// does.
struct SavedDatabasesAreDown(Fake);

impl Handler for SavedDatabasesAreDown {
    async fn serve(&self, command: Command, reply: Reply) {
        if let Command::UseDatabase { request, database } = &command
            && !database.is_local()
        {
            let failure = Failure::qdrant_down(&database.qdrant_url);
            return reply.send(Event::DatabaseUsed {
                request: *request,
                result: Err(failure),
            });
        }
        self.0.serve(command, reply).await;
    }
}

fn a_switch_that_fails_changes_nothing_but_the_sheet() {
    const ADDRESS: &str = "http://127.0.0.1:16334";
    let (mut harness, _) = recording::open_with("idle", DEFAULT_WINDOW, SavedDatabasesAreDown);
    testkit::settle(&mut harness);
    database_entry(&harness, "Database: Local · connected")
        .expect("the entry is there")
        .click();
    testkit::settle(&mut harness);
    click(&mut harness, Role::ComboBox, "Database");
    click(&mut harness, Role::Button, "New…");
    type_into(&mut harness, "Name", "Office server");
    type_into(&mut harness, "Qdrant URL", ADDRESS);
    type_into(&mut harness, "FalkorDB URL", "falkor://127.0.0.1:16379");
    let folder = testkit::samples_folder().to_string_lossy().into_owned();
    type_into(&mut harness, "Content folder", &folder);
    click(&mut harness, Role::Button, "Use");

    assert!(
        is_a_sheet_open(&mut harness),
        "a failed switch closed the sheet"
    );
    let hint = Failure::qdrant_down(ADDRESS).hint;
    assert!(the_sheet_says(&harness, &hint), "the sheet lacks `{hint}`");
    assert!(database_entry(&harness, "Database: Local · connected").is_some());
    let shared = shared(&harness);
    assert!(
        !matches!(shared.health.of(Service::Qdrant), ServiceState::Down(_)),
        "a failed switch marked the Qdrant in use as down"
    );
    assert_eq!(
        shared
            .database
            .in_use
            .as_ref()
            .map(|database| database.name.as_str()),
        Some(LOCAL_DATABASE)
    );
}

#[test]
fn the_database_is_chosen_before_anything_loads_and_the_footer_says_which() {
    let home = tempfile::tempdir().expect("a temporary home");
    let mut harness = first_start_in(home.path());
    the_first_start_asks_which_database_before_anything_loads(&mut harness);
    using_the_database_saves_the_choice_and_names_it_on_every_tab(&mut harness, home.path());
    a_database_that_is_down_says_so_in_the_footer_and_in_the_sheet();
    no_switch_lands_under_a_running_ingest();
    let another_home = tempfile::tempdir().expect("a temporary home");
    a_key_typed_into_the_sheet_is_in_no_node_but_is_saved(another_home.path());
    a_switch_that_fails_changes_nothing_but_the_sheet();
}
