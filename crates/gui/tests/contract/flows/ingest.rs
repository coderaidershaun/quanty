//! A chapter is checked and started from the Ingest page and runs to its end on the fake backend,
//! the catalogue is loaded again when it is stored, and a start that fails says so with its hint.

use std::path::PathBuf;

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::{DEFAULT_WINDOW, MIN_WINDOW};
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{Book, Command, IngestRequest, Intent, NewBook, Tab};
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
    let book = node(&harness, Role::ComboBox, "Book").value();
    assert_eq!(book.as_deref(), Some("Option Volatility and Pricing"));
    assert!(
        !has(&harness, Role::TextInput, "Book title"),
        "a book of the library is chosen from the list, not typed"
    );
    let form = ["Sheldon Natenberg", "options, volatility"];
    for (name, text) in ["Author", "Tags"].into_iter().zip(form) {
        assert_eq!(field(&harness, name).as_deref(), Some(text), "{name}");
    }
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
    assert!(says(&harness, "Add a chapter"));
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
    matches!(command, Command::SaveBook { .. })
}

fn is_catalogue_load(command: &Command) -> bool {
    matches!(command, Command::LoadCatalogue { .. })
}

fn saved_books(seen: &Seen) -> Vec<NewBook> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::SaveBook { book, .. } => Some(book),
            _ => None,
        })
        .collect()
}

fn open_the_form_of_a_new_book(harness: &mut Window) {
    click(harness, Role::ComboBox, "Book");
    click(harness, Role::Button, "Add a new book…");
}

fn choose_in_the_book_list(harness: &mut Window, title: &str) {
    click(harness, Role::ComboBox, "Book");
    click(harness, Role::Button, title);
}

fn close_the_open_list(harness: &mut Window) {
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
}

const PLACEHOLDER_OF_THE_BOOK_LIST: &str = "Choose a book";

fn cancel_clears_the_form_of_a_new_book_and_sends_nothing(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Tab, "Ingest");
    open_the_form_of_a_new_book(harness);
    assert!(has(harness, Role::Button, "Save book"));
    assert!(has(harness, Role::Button, "Cancel"));
    assert!(
        !has(harness, Role::Button, "Choose from the library"),
        "Cancel is the one way back"
    );
    type_into(harness, "Book title", "Natenberg on Options");
    type_into(harness, "Author", "Sheldon Natenberg");
    type_into(harness, "Tags", "Volatility, options");
    testkit::save_png(harness, "app-ingest-new-book-cancel");

    let sent = seen.all().len();
    click(harness, Role::Button, "Cancel");

    assert_eq!(seen.all().len(), sent, "Cancel sends no command");
    assert_eq!(seen.count(is_save), 0);
    assert!(!has(harness, Role::TextInput, "Book title"));
    for name in ["Author", "Tags"] {
        assert_eq!(field(harness, name).as_deref(), Some(""), "{name}");
    }
    let book = node(harness, Role::ComboBox, "Book").value();
    assert_eq!(book.as_deref(), Some(PLACEHOLDER_OF_THE_BOOK_LIST));
    assert!(!has(harness, Role::Button, "Save book"));
    assert!(!is_enabled(harness, Role::Button, "Check the chapter"));
}

fn a_new_book_is_typed_and_saved_with_no_pdf(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Tab, "Ingest");
    open_the_form_of_a_new_book(harness);
    assert!(has(harness, Role::Button, "Save book"));
    assert!(
        !is_enabled(harness, Role::Button, "Save book"),
        "a book with no title cannot be saved"
    );

    type_into(harness, "Book title", "Natenberg on Options");
    type_into(harness, "Author", "Sheldon Natenberg");
    type_into(harness, "Tags", "Volatility, options");
    click(harness, Role::Button, "Save book");

    let wanted = NewBook {
        title: "Natenberg on Options".to_owned(),
        author: Some("Sheldon Natenberg".to_owned()),
        tags: vec!["Volatility".to_owned(), "options".to_owned()],
    };
    assert_eq!(saved_books(seen), [wanted]);
    assert_eq!(seen.count(is_preflight), 0, "no PDF was chosen");
    assert_eq!(
        seen.count(is_catalogue_load),
        2,
        "the catalogue is loaded at the start and again after the save"
    );
    let book = node(harness, Role::ComboBox, "Book").value();
    assert_eq!(book.as_deref(), Some("Natenberg on Options"));
    assert!(!has(harness, Role::TextInput, "Book title"));
    assert_eq!(
        field(harness, "Author").as_deref(),
        Some("Sheldon Natenberg")
    );
    assert_eq!(
        field(harness, "Tags").as_deref(),
        Some("options, volatility")
    );
    testkit::save_png(harness, "app-ingest-new-book-saved");
}

fn the_saved_book_fills_author_and_tags_each_time_it_is_chosen(harness: &mut Window) {
    choose_in_the_book_list(harness, "Quanty Sample Notes");
    assert_ne!(
        field(harness, "Tags").as_deref(),
        Some("options, volatility")
    );

    choose_in_the_book_list(harness, "Natenberg on Options");

    assert_eq!(
        field(harness, "Author").as_deref(),
        Some("Sheldon Natenberg")
    );
    assert_eq!(
        field(harness, "Tags").as_deref(),
        Some("options, volatility")
    );
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

fn a_chapter_of_the_saved_book_is_labelled_with_what_was_saved(harness: &mut Window, seen: &Seen) {
    harness
        .state_mut()
        .push(Intent::PdfPicked(PathBuf::from("chapter-1-basics.pdf")));
    testkit::settle(harness);
    click(harness, Role::Button, "Check the chapter");

    let ingest = last_checked(seen);
    assert_eq!(ingest.book, "Natenberg on Options");
    assert_eq!(ingest.author.as_deref(), Some("Sheldon Natenberg"));
    assert_eq!(ingest.tags, ["options", "volatility"]);
}

fn a_book_with_no_chapter_is_not_a_filter_and_not_in_the_source_pickers(harness: &mut Window) {
    click(harness, Role::Tab, "Ask");
    click(harness, Role::ComboBox, "Books");
    assert!(has(harness, Role::Button, "All books"));
    assert!(!has(harness, Role::Button, "Natenberg on Options"));
    close_the_open_list(harness);

    click(harness, Role::ComboBox, "Book");
    assert!(
        has(harness, Role::Button, "Quanty Sample Notes"),
        "the list of the Source panel is open"
    );
    assert!(!has(harness, Role::Button, "Natenberg on Options"));
    close_the_open_list(harness);
}

fn a_second_book_with_the_same_title_is_refused_by_name(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Tab, "Ingest");
    open_the_form_of_a_new_book(harness);
    type_into(harness, "Book title", "  natenberg ON options ");
    click(harness, Role::Button, "Save book");
    assert!(says(
        harness,
        "Natenberg on Options is in the library already"
    ));
    // A second book would have the title as it was typed, so capitals are not compared.
    let catalogue = shared(harness).library.catalogue.ready();
    let books = catalogue.map_or(0, |catalogue| {
        let is_the_book = |book: &&Book| {
            let title = book.title.as_deref();
            title.is_some_and(|title| title.eq_ignore_ascii_case("Natenberg on Options"))
        };
        catalogue.books.iter().filter(is_the_book).count()
    });
    assert_eq!(books, 1, "the book was not saved a second time");

    click(harness, Role::Button, "Cancel");
    open_the_form_of_a_new_book(harness);
    type_into(harness, "Book title", "option volatility and pricing");
    click(harness, Role::Button, "Save book");
    assert!(says(
        harness,
        "Option Volatility and Pricing is in the library already"
    ));

    assert_eq!(seen.count(is_save), 3);
    assert_eq!(
        seen.count(is_catalogue_load),
        2,
        "a refusal changes nothing"
    );
}

fn cancel_clears_a_check_that_was_made_for_the_new_book(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Tab, "Ingest");
    click(harness, Role::Button, "Cancel");
    open_the_form_of_a_new_book(harness);
    type_into(harness, "Book title", "Dynamic Hedging");
    harness
        .state_mut()
        .push(Intent::PdfPicked(PathBuf::from("chapter-2-hedging.pdf")));
    testkit::settle(harness);
    let checks = seen.count(is_preflight);
    click(harness, Role::Button, "Check the chapter");
    assert_eq!(seen.count(is_preflight), checks + 1);
    assert!(
        matches!(shared(harness).ingest, IngestJob::Checked { .. }),
        "{:?}",
        shared(harness).ingest
    );
    assert!(has(harness, Role::Button, "Start ingest"));

    click(harness, Role::Button, "Cancel");

    assert_eq!(
        shared(harness).ingest,
        IngestJob::Idle,
        "the old check is gone"
    );
    assert!(!has(harness, Role::Button, "Start ingest"));
    let book = node(harness, Role::ComboBox, "Book").value();
    assert_eq!(book.as_deref(), Some(PLACEHOLDER_OF_THE_BOOK_LIST));
    assert!(!is_enabled(harness, Role::Button, "Check the chapter"));

    choose_in_the_book_list(harness, "Quanty Sample Notes");
    assert!(is_enabled(harness, Role::Button, "Check the chapter"));
    click(harness, Role::Button, "Check the chapter");
    assert_eq!(seen.count(is_preflight), checks + 2);
    assert!(matches!(shared(harness).ingest, IngestJob::Checked { .. }));
}

fn a_typed_title_of_a_stored_book_is_sent_as_the_library_has_it() {
    let (mut harness, seen) = open("idle", DEFAULT_WINDOW);
    click(&mut harness, Role::Tab, "Ingest");
    open_the_form_of_a_new_book(&mut harness);
    type_into(
        &mut harness,
        "Book title",
        " option volatility AND pricing ",
    );
    harness
        .state_mut()
        .push(Intent::PdfPicked(PathBuf::from("chapter-4-vega.pdf")));
    testkit::settle(&mut harness);
    click(&mut harness, Role::Button, "Check the chapter");

    assert_eq!(
        last_checked(&seen).book,
        "Option Volatility and Pricing",
        "the title as it was typed would start a second book"
    );
    assert!(
        matches!(shared(&harness).ingest, IngestJob::Checked { .. }),
        "the check stands: {:?}",
        shared(&harness).ingest
    );
}

fn with_no_book_in_the_library_the_list_is_there_to_go_back_to() {
    let (mut harness, _) = open("empty-library", DEFAULT_WINDOW);
    click(&mut harness, Role::Tab, "Ingest");
    assert!(
        !has(&harness, Role::TextInput, "Book title"),
        "the box for a title does not open by itself"
    );
    assert!(says(&harness, "No book is stored yet."));
    let book = node(&harness, Role::ComboBox, "Book").value();
    assert_eq!(book.as_deref(), Some(PLACEHOLDER_OF_THE_BOOK_LIST));

    open_the_form_of_a_new_book(&mut harness);
    assert!(has(&harness, Role::TextInput, "Book title"));
    assert!(has(&harness, Role::Button, "Save book"));
    assert!(has(&harness, Role::Button, "Cancel"));

    click(&mut harness, Role::Button, "Cancel");
    assert!(!has(&harness, Role::TextInput, "Book title"));
    assert!(has(&harness, Role::ComboBox, "Book"));
}

/// Answers every command but the save of a book, so that save never ends.
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
    open_the_form_of_a_new_book(&mut harness);
    type_into(&mut harness, "Book title", "Dynamic Hedging");
    assert!(is_enabled(&harness, Role::Button, "Cancel"));
    // The save never ends, so the app is never idle again, and `click` and `settle` wait for an
    // idle app.
    node(&harness, Role::Button, "Save book").click();
    harness.run_ok();
    assert!(
        !is_enabled(&harness, Role::Button, "Cancel"),
        "a save that runs cannot be given up"
    );
    assert!(has(&harness, Role::TextInput, "Book title"));
}

#[test]
fn a_book_is_saved_with_no_pdf_is_offered_after_and_is_refused_a_second_time() {
    let (mut harness, seen) = open("idle", DEFAULT_WINDOW);
    cancel_clears_the_form_of_a_new_book_and_sends_nothing(&mut harness, &seen);
    a_new_book_is_typed_and_saved_with_no_pdf(&mut harness, &seen);
    the_saved_book_fills_author_and_tags_each_time_it_is_chosen(&mut harness);
    a_chapter_of_the_saved_book_is_labelled_with_what_was_saved(&mut harness, &seen);
    a_book_with_no_chapter_is_not_a_filter_and_not_in_the_source_pickers(&mut harness);
    a_second_book_with_the_same_title_is_refused_by_name(&mut harness, &seen);
    cancel_clears_a_check_that_was_made_for_the_new_book(&mut harness, &seen);
    a_typed_title_of_a_stored_book_is_sent_as_the_library_has_it();
    with_no_book_in_the_library_the_list_is_there_to_go_back_to();
    cancel_is_off_while_the_save_runs();
}
