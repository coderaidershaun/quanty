//! A media is saved before its first PDF, is shown after it as a card whose labels only its
//! pencil changes, and is refused a second time, and a paper's PDF of any name is checked by
//! itself and ingested.

use std::path::PathBuf;

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{
    Category, Command, DocumentName, IngestRequest, Intent, Media, MediaEdit, NewMedia,
};
use gui::state::IngestJob;
use gui::testkit;

use super::{
    choose_in_the_media_list, close_the_open_list, is_preflight, last_checked, open,
    open_the_form_of_a_new_media,
};
use crate::flows::recording::{self, Seen};
use crate::flows::{
    Window, click, field, has, is_enabled, node, press, retype, says, shared, type_into,
};

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

fn edited_media(seen: &Seen) -> Vec<MediaEdit> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::EditMedia { edit, .. } => Some(edit),
            _ => None,
        })
        .collect()
}

const PLACEHOLDER_OF_THE_MEDIA_LIST: &str = "Choose a media";
const SAVED_PAPER: &str = "Rough Volatility";
const ROW_OF_THE_SAVED_PAPER: &str = "Rough Volatility · Paper";
const ITS_AUTHORS: &str = "A. Author, B. Author";

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
    assert_eq!(seen.count(is_preflight), 0, "nothing was checked");
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
    assert_eq!(media.as_deref(), Some(ROW_OF_THE_SAVED_PAPER));
    for name in ["Media title", "Authors", "Tags"] {
        assert!(
            !has(harness, Role::TextInput, name),
            "the labels of a saved media are fixed on its card: `{name}` is a box"
        );
    }
    assert!(
        has(harness, Role::Label, "Paper"),
        "the card names the category"
    );
    assert!(says(harness, ITS_AUTHORS));
    for tag in ["rough", "volatility"] {
        assert!(has(harness, Role::Label, tag), "the tag `{tag}` is shown");
    }
    testkit::save_png(harness, "app-ingest-new-book-saved");
}

fn the_saved_media_shows_its_labels_each_time_it_is_chosen(harness: &mut Window) {
    choose_in_the_media_list(harness, "Quanty Sample Notes · Book");
    assert!(says(harness, "Quanty Team"));
    assert!(has(harness, Role::Label, "Book"));
    assert!(!says(harness, ITS_AUTHORS));

    choose_in_the_media_list(harness, ROW_OF_THE_SAVED_PAPER);
    assert!(says(harness, ITS_AUTHORS));
}

fn the_pencil_edits_the_chosen_media(harness: &mut Window, seen: &Seen) {
    click(harness, Role::Button, "Edit media");
    assert_eq!(field(harness, "Authors").as_deref(), Some(ITS_AUTHORS));
    assert!(
        !is_enabled(harness, Role::TextInput, "Media title"),
        "the title names the folder of the media, so it cannot change"
    );
    retype(harness, "Tags", "rough");
    click(harness, Role::Button, "Save media");

    let wanted = MediaEdit {
        title: SAVED_PAPER.to_owned(),
        category: Category::Paper,
        authors: vec!["A. Author".to_owned(), "B. Author".to_owned()],
        tags: vec!["rough".to_owned()],
    };
    assert_eq!(edited_media(seen), [wanted]);
    assert!(!has(harness, Role::TextInput, "Tags"), "the form is closed");
    assert!(has(harness, Role::Label, "rough"));
    assert!(
        !has(harness, Role::Label, "volatility"),
        "the card shows the tags of the edit"
    );
}

fn a_paper_pdf_of_the_saved_paper_is_checked_and_ingested(harness: &mut Window, seen: &Seen) {
    let checks = seen.count(is_preflight);
    harness
        .state_mut()
        .push(Intent::PdfPicked(PathBuf::from("hawkes-notes.pdf")));
    testkit::settle(harness);

    assert_eq!(
        seen.count(is_preflight),
        checks + 1,
        "the pick is checked with no click"
    );
    let mut wanted = IngestRequest {
        pdf: PathBuf::from("hawkes-notes.pdf"),
        media: SAVED_PAPER.to_owned(),
        category: Category::Paper,
        name: DocumentName::Title(SAVED_PAPER.to_owned()),
        tags: Vec::new(),
    };
    assert_eq!(
        last_checked(seen),
        wanted,
        "a paper's PDF is named after its media, with nothing typed"
    );

    type_into(harness, "Tags for this PDF", "Hawkes, intensity");
    press(harness, egui::Modifiers::NONE, egui::Key::Enter);

    assert_eq!(seen.count(is_preflight), checks + 2);
    wanted.tags = vec!["Hawkes".to_owned(), "intensity".to_owned()];
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
    click(harness, Role::ComboBox, "Books");
    assert!(has(harness, Role::Button, "All books"));
    assert!(!has(harness, Role::Button, SAVED_PAPER));
    close_the_open_list(harness);

    click(harness, Role::ComboBox, "Book");
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
    the_pencil_edits_the_chosen_media(&mut harness, &seen);
    a_paper_pdf_of_the_saved_paper_is_checked_and_ingested(&mut harness, &seen);
    a_media_with_no_document_is_not_a_filter_and_not_in_the_source_pickers(&mut harness);
    a_second_media_with_the_same_title_is_refused_by_name(&mut harness, &seen);
    with_no_media_in_the_library_the_list_is_there_to_go_back_to();
    cancel_is_off_while_the_save_runs();
}
