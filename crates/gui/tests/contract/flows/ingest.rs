//! A chapter is checked and started from the Ingest page and runs to its end on the fake backend,
//! the catalogue is loaded again when it is stored, and a start that fails says so with its hint. A
//! media is saved before its first PDF, and a paper's PDF of any name is checked and ingested.

use std::path::PathBuf;

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::{DEFAULT_WINDOW, MIN_WINDOW};
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Category, Command, DocumentName, IngestRequest, Intent, Media, NewMedia, Tab};
use gui::state::IngestJob;
use gui::testkit;

use super::recording::{self, Seen};
use super::{COMMAND, Window, click, field, has, is_enabled, node, press, says, shared, type_into};

fn is_preflight(command: &Command) -> bool {
    matches!(command, Command::Preflight { .. })
}

fn is_ingest(command: &Command) -> bool {
    matches!(command, Command::Ingest { .. })
}

fn open(scene: &str, size: [f32; 2]) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, size);
    testkit::settle(&mut harness);
    (harness, seen)
}

fn place_of(seen: &Seen, is: fn(&Command) -> bool) -> Option<usize> {
    seen.all().iter().position(is)
}

fn a_checked_chapter_runs_to_done_and_loads_the_catalogue_again() {
    let (mut harness, seen) = open("ingest-ready", DEFAULT_WINDOW);
    assert_eq!(shared(&harness).tab, Tab::Ingest);
    let media = node(&harness, Role::ComboBox, "Media").value();
    assert_eq!(media.as_deref(), Some("Option Volatility and Pricing"));
    assert!(
        !has(&harness, Role::TextInput, "Media title"),
        "a media of the library is chosen from the list, not typed"
    );
    assert!(
        !has(&harness, Role::TextInput, "Authors"),
        "the labels of a chosen media come from the media, not from the form"
    );
    assert_eq!(
        field(&harness, "Tags for this PDF").as_deref(),
        Some("greeks")
    );
    assert!(says(&harness, "Chapter 3 · Greeks"));
    assert!(has(&harness, Role::Button, "Start ingest"));
    testkit::save_png(&mut harness, "app-ingest-ready");

    click(&mut harness, Role::Button, "Start ingest");
    let IngestJob::Finished { result, .. } = &shared(&harness).ingest else {
        panic!("the ingest did not finish: {:?}", shared(&harness).ingest);
    };
    assert!(result.is_ok(), "{result:?}");
    assert!(says(
        &harness,
        "Ingested Option Volatility and Pricing, chapter 3: Greeks"
    ));
    assert!(has(&harness, Role::Button, "Add another chapter"));
    testkit::save_png(&mut harness, "app-ingest-done");

    assert_eq!(seen.count(is_preflight), 1);
    assert_eq!(seen.count(is_ingest), 1);
    assert!(place_of(&seen, is_preflight) < place_of(&seen, is_ingest));
    assert_eq!(
        seen.count(|command| matches!(command, Command::LoadCatalogue { .. })),
        2,
        "the catalogue is loaded at the start and again when the chapter is stored"
    );
}

fn a_start_that_fails_says_so_and_try_again_checks_again() {
    let (mut harness, seen) = open("ingest-failed", DEFAULT_WINDOW);
    click(&mut harness, Role::Button, "Start ingest");
    let IngestJob::Finished {
        result: Err(failure),
        ..
    } = &shared(&harness).ingest
    else {
        panic!("the ingest did not fail: {:?}", shared(&harness).ingest);
    };
    let hint = failure.hint.clone();
    assert!(says(&harness, "The ingest failed"));
    assert!(says(&harness, &hint));
    testkit::save_png(&mut harness, "app-ingest-failed");
    assert_eq!(
        seen.count(|command| matches!(command, Command::LoadCatalogue { .. })),
        1,
        "a failed ingest stored nothing, so the catalogue stays as it is"
    );

    click(&mut harness, Role::Button, "Try again");
    assert_eq!(seen.count(is_preflight), 2);
    assert!(
        matches!(shared(&harness).ingest, IngestJob::Checked { .. }),
        "the second check came back"
    );
}

fn the_tab_and_the_keys_open_the_page_and_go_back() {
    let (mut harness, seen) = open("idle", DEFAULT_WINDOW);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    click(&mut harness, Role::Tab, "Ingest");
    assert_eq!(shared(&harness).tab, Tab::Ingest);
    assert!(says(&harness, "Add media"));
    testkit::save_png(&mut harness, "app-idle-tabs");

    press(&mut harness, COMMAND, egui::Key::Num1);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    press(&mut harness, COMMAND, egui::Key::Num3);
    assert_eq!(shared(&harness).tab, Tab::Ingest);
    press(&mut harness, COMMAND, egui::Key::Num1);
    assert_eq!(shared(&harness).tab, Tab::Ask);
    assert!(
        seen.all()
            .iter()
            .all(|command| !is_preflight(command) && !is_ingest(command)),
        "opening the page checks nothing"
    );
}

fn the_page_fits_the_smallest_window() {
    let (mut harness, _) = open("ingest-ready", MIN_WINDOW);
    assert!(has(&harness, Role::Button, "Start ingest"));
    testkit::save_png(&mut harness, "app-ingest-ready-min");
}

#[test]
fn a_checked_chapter_runs_to_done_on_the_fake_and_the_catalogue_is_loaded_again() {
    a_checked_chapter_runs_to_done_and_loads_the_catalogue_again();
    a_start_that_fails_says_so_and_try_again_checks_again();
    the_tab_and_the_keys_open_the_page_and_go_back();
    the_page_fits_the_smallest_window();
}

fn is_save(command: &Command) -> bool {
    matches!(command, Command::SaveMedia { .. })
}

fn is_catalogue_load(command: &Command) -> bool {
    matches!(command, Command::LoadCatalogue { .. })
}

fn saved_media(seen: &Seen) -> Vec<NewMedia> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::SaveMedia { media, .. } => Some(media),
            _ => None,
        })
        .collect()
}

fn open_the_form_of_a_new_media(harness: &mut Window) {
    click(harness, Role::ComboBox, "Media");
    click(harness, Role::Button, "Add new media…");
}

fn choose_in_the_media_list(harness: &mut Window, title: &str) {
    click(harness, Role::ComboBox, "Media");
    click(harness, Role::Button, title);
}

fn close_the_open_list(harness: &mut Window) {
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
}

const PLACEHOLDER_OF_THE_MEDIA_LIST: &str = "Choose a media";
const SAVED_PAPER: &str = "Rough Volatility";

fn cancel_clears_the_form_of_a_new_media_and_sends_nothing(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Tab, "Ingest");
    open_the_form_of_a_new_media(harness);
    assert!(has(harness, Role::Button, "Save media"));
    assert!(has(harness, Role::Button, "Cancel"));
    assert!(
        !has(harness, Role::Button, "Choose from the library"),
        "Cancel is the one way back"
    );
    type_into(harness, "Media title", "Natenberg on Options");
    type_into(harness, "Authors", "Sheldon Natenberg");
    type_into(harness, "Tags", "Volatility, options");
    testkit::save_png(harness, "app-ingest-new-book-cancel");

    let sent = seen.all().len();
    click(harness, Role::Button, "Cancel");

    assert_eq!(seen.all().len(), sent, "Cancel sends no command");
    assert_eq!(seen.count(is_save), 0);
    for name in ["Media title", "Authors", "Tags"] {
        assert!(!has(harness, Role::TextInput, name), "{name}");
    }
    let media = node(harness, Role::ComboBox, "Media").value();
    assert_eq!(media.as_deref(), Some(PLACEHOLDER_OF_THE_MEDIA_LIST));
    assert!(!has(harness, Role::Button, "Save media"));
    assert!(!is_enabled(harness, Role::Button, "Check the chapter"));
}

fn a_new_paper_is_typed_and_saved_with_no_pdf(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Tab, "Ingest");
    open_the_form_of_a_new_media(harness);
    assert!(has(harness, Role::Button, "Save media"));
    assert!(
        !is_enabled(harness, Role::Button, "Save media"),
        "a media with no title cannot be saved"
    );

    click(harness, Role::ComboBox, "Category");
    click(harness, Role::Button, "Paper");
    type_into(harness, "Media title", SAVED_PAPER);
    type_into(harness, "Authors", "A. Author, B. Author");
    type_into(harness, "Tags", "Volatility, rough");
    click(harness, Role::Button, "Save media");

    let wanted = NewMedia {
        title: SAVED_PAPER.to_owned(),
        category: Category::Paper,
        authors: vec!["A. Author".to_owned(), "B. Author".to_owned()],
        tags: vec!["Volatility".to_owned(), "rough".to_owned()],
    };
    assert_eq!(saved_media(seen), [wanted]);
    assert_eq!(seen.count(is_preflight), 0, "no PDF was chosen");
    assert_eq!(
        seen.count(is_catalogue_load),
        2,
        "the catalogue is loaded at the start and again after the save"
    );
    let media = node(harness, Role::ComboBox, "Media").value();
    assert_eq!(media.as_deref(), Some(SAVED_PAPER));
    assert!(!has(harness, Role::TextInput, "Media title"));
    assert!(says(harness, "Paper · A. Author, B. Author"));
    for tag in ["rough", "volatility"] {
        assert!(has(harness, Role::Label, tag), "the tag `{tag}` is shown");
    }
    testkit::save_png(harness, "app-ingest-new-book-saved");
}

fn the_saved_media_shows_its_labels_each_time_it_is_chosen(harness: &mut Window) {
    choose_in_the_media_list(harness, "Quanty Sample Notes");
    assert!(says(harness, "Book · Quanty Team"));
    assert!(!says(harness, "Paper · A. Author, B. Author"));

    choose_in_the_media_list(harness, SAVED_PAPER);
    assert!(says(harness, "Paper · A. Author, B. Author"));
}

fn last_checked(seen: &Seen) -> IngestRequest {
    let checked = seen
        .all()
        .into_iter()
        .rev()
        .find_map(|command| match command {
            Command::Preflight { ingest, .. } => Some(ingest),
            _ => None,
        });
    checked.expect("the chapter was not checked")
}

fn a_paper_pdf_of_the_saved_paper_is_checked_and_ingested(harness: &mut Window, seen: &Seen) {
    harness
        .state_mut()
        .push(Intent::PdfPicked(PathBuf::from("hawkes-notes.pdf")));
    testkit::settle(harness);
    assert!(
        !says(harness, "must be named chapter-"),
        "the file-name rule is for a book"
    );
    type_into(harness, "Tags for this PDF", "Hawkes, intensity");
    click(harness, Role::Button, "Check the chapter");

    let wanted = IngestRequest {
        pdf: PathBuf::from("hawkes-notes.pdf"),
        media: SAVED_PAPER.to_owned(),
        category: Category::Paper,
        name: DocumentName::Title(SAVED_PAPER.to_owned()),
        tags: vec!["Hawkes".to_owned(), "intensity".to_owned()],
    };
    assert_eq!(last_checked(seen), wanted);
    let loads = seen.count(is_catalogue_load);
    click(harness, Role::Button, "Start ingest");
    let IngestJob::Finished { result, .. } = &shared(harness).ingest else {
        panic!("the ingest did not finish: {:?}", shared(harness).ingest);
    };
    assert!(result.is_ok(), "{result:?}");
    assert!(says(harness, &format!("Ingested {SAVED_PAPER}")));
    assert_eq!(
        seen.count(is_catalogue_load),
        loads + 1,
        "the catalogue is loaded again when the document is stored"
    );
}

fn a_media_with_no_document_is_not_a_filter_and_not_in_the_source_pickers(harness: &mut Window) {
    click(harness, Role::Tab, "Ask");
    let panels = super::panels(DEFAULT_WINDOW);
    super::click_in(harness, Role::ComboBox, "Media", panels.ask_bar);
    assert!(has(harness, Role::Button, "All media"));
    assert!(!has(harness, Role::Button, SAVED_PAPER));
    close_the_open_list(harness);

    super::click_in(harness, Role::ComboBox, "Media", panels.source);
    assert!(
        has(harness, Role::Button, "Quanty Sample Notes"),
        "the list of the Source panel is open"
    );
    assert!(!has(harness, Role::Button, SAVED_PAPER));
    close_the_open_list(harness);
}

fn a_second_media_with_the_same_title_is_refused_by_name(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Tab, "Ingest");
    let loads = seen.count(is_catalogue_load);
    open_the_form_of_a_new_media(harness);
    type_into(harness, "Media title", "  rough VOLATILITY ");
    click(harness, Role::Button, "Save media");
    assert!(says(harness, "Rough Volatility is in the library already"));
    // A second media would have the title as it was typed, so capitals are not compared.
    let catalogue = shared(harness).library.catalogue.ready();
    let copies = catalogue.map_or(0, |catalogue| {
        let is_the_paper = |media: &&Media| {
            let title = media.title.as_deref();
            title.is_some_and(|title| title.eq_ignore_ascii_case(SAVED_PAPER))
        };
        catalogue.media.iter().filter(is_the_paper).count()
    });
    assert_eq!(copies, 1, "the media was not saved a second time");

    click(harness, Role::Button, "Cancel");
    open_the_form_of_a_new_media(harness);
    type_into(harness, "Media title", "option volatility and pricing");
    click(harness, Role::Button, "Save media");
    assert!(says(
        harness,
        "Option Volatility and Pricing is in the library already"
    ));

    assert_eq!(seen.count(is_save), 3);
    assert_eq!(
        seen.count(is_catalogue_load),
        loads,
        "a refusal changes nothing"
    );
}

fn with_no_media_in_the_library_the_list_is_there_to_go_back_to() {
    let (mut harness, _) = open("empty-library", DEFAULT_WINDOW);
    click(&mut harness, Role::Tab, "Ingest");
    assert!(
        !has(&harness, Role::TextInput, "Media title"),
        "the box for a title does not open by itself"
    );
    assert!(says(&harness, "No media is stored yet."));
    let media = node(&harness, Role::ComboBox, "Media").value();
    assert_eq!(media.as_deref(), Some(PLACEHOLDER_OF_THE_MEDIA_LIST));

    open_the_form_of_a_new_media(&mut harness);
    assert!(has(&harness, Role::TextInput, "Media title"));
    assert!(has(&harness, Role::Button, "Save media"));
    assert!(has(&harness, Role::Button, "Cancel"));

    click(&mut harness, Role::Button, "Cancel");
    assert!(!has(&harness, Role::TextInput, "Media title"));
    assert!(has(&harness, Role::ComboBox, "Media"));
}

/// Answers every command but the save of a media, so that save never ends.
struct SaveNeverEnds(Fake);

impl Handler for SaveNeverEnds {
    async fn serve(&self, command: Command, reply: Reply) {
        if !is_save(&command) {
            self.0.serve(command, reply).await;
        }
    }
}

fn cancel_is_off_while_the_save_runs() {
    let (mut harness, _) = recording::open_with("idle", DEFAULT_WINDOW, SaveNeverEnds);
    testkit::settle(&mut harness);
    click(&mut harness, Role::Tab, "Ingest");
    open_the_form_of_a_new_media(&mut harness);
    type_into(&mut harness, "Media title", "Dynamic Hedging");
    assert!(is_enabled(&harness, Role::Button, "Cancel"));
    // The save never ends, so the app is never idle again, and `click` and `settle` wait for an
    // idle app.
    node(&harness, Role::Button, "Save media").click();
    harness.run_ok();
    assert!(
        !is_enabled(&harness, Role::Button, "Cancel"),
        "a save that runs cannot be given up"
    );
    assert!(has(&harness, Role::TextInput, "Media title"));
}

#[test]
fn a_book_is_saved_with_no_pdf_is_offered_after_and_is_refused_a_second_time() {
    let (mut harness, seen) = open("idle", DEFAULT_WINDOW);
    cancel_clears_the_form_of_a_new_media_and_sends_nothing(&mut harness, &seen);
    a_new_paper_is_typed_and_saved_with_no_pdf(&mut harness, &seen);
    the_saved_media_shows_its_labels_each_time_it_is_chosen(&mut harness);
    a_paper_pdf_of_the_saved_paper_is_checked_and_ingested(&mut harness, &seen);
    a_media_with_no_document_is_not_a_filter_and_not_in_the_source_pickers(&mut harness);
    a_second_media_with_the_same_title_is_refused_by_name(&mut harness, &seen);
    with_no_media_in_the_library_the_list_is_there_to_go_back_to();
    cancel_is_off_while_the_save_runs();
}
