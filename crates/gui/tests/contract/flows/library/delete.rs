//! A Delete button on the card of a document or of a media asks first, in a sheet, and only its
//! confirm sends the delete. The card is gone once the library is read again, and a delete that
//! failed says why on its card.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use gui::contract::{
    Catalogue, Category, Command, DocId, Document, DocumentName, FailureKind, IngestRequest,
    Intent, NewMedia, NoticeKind, Tab,
};
use gui::state::{IngestJob, MediaDelete};
use gui::testkit;
use uuid::Uuid;

use super::{Ingests, REFUSED_HINT, click_on_the_card, documents_of, open_switched};
use crate::flows::recording::Seen;
use crate::flows::{COMMAND, Window, click, has, press, says, shared};

const SAMPLE_PAGES: &str = "Chapter 1 · Sample Pages";
const NOTES: &str = "Quanty Sample Notes";
const BOOK: &str = "Option Volatility and Pricing";

fn the_document(harness: &Window, label: &str) -> Document {
    documents_of(harness)
        .into_iter()
        .find(|document| document.name().label() == label)
        .unwrap_or_else(|| panic!("the library has no document `{label}`"))
}

fn the_catalogue(harness: &Window) -> Catalogue {
    let catalogue = shared(harness).library.catalogue.ready().cloned();
    catalogue.expect("the catalogue is loaded")
}

fn delete_document_button(label: &str) -> String {
    format!("Delete document {label}")
}

fn delete_media_button(title: &str) -> String {
    format!("Delete media {title}")
}

fn sent_document_deletes(seen: &Seen) -> Vec<DocId> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::DeleteDocument { doc, .. } => Some(doc),
            _ => None,
        })
        .collect()
}

fn sent_media_deletes(seen: &Seen) -> Vec<String> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::DeleteMedia { title, .. } => Some(title),
            _ => None,
        })
        .collect()
}

fn nothing_was_sent(seen: &Seen) {
    assert_eq!(sent_document_deletes(seen), []);
    assert_eq!(sent_media_deletes(seen), Vec::<String>::new());
}

fn loads_of_the_catalogue(seen: &Seen) -> usize {
    seen.count(|command| matches!(command, Command::LoadCatalogue { .. }))
}

fn every_document_has_one_delete_button(harness: &Window) {
    let documents = documents_of(harness);
    assert!(!documents.is_empty());
    for document in documents {
        let name = delete_document_button(&document.name().label());
        let buttons = harness.query_all_by_role_and_label(Role::Button, &name);
        assert_eq!(buttons.count(), 1, "`{name}` is one button");
    }
}

fn pressing_delete_opens_a_sheet_that_says_what_goes(harness: &mut Window) {
    click_on_the_card(harness, Role::Button, &delete_document_button(SAMPLE_PAGES));
    assert!(
        has(
            harness,
            Role::Label,
            "Delete the document Chapter 1 · Sample Pages?"
        ),
        "the sheet names the document"
    );
    for words in [
        "passages, formulas, figures and tables",
        "converted pages",
        "is not touched",
        "cannot be undone",
    ] {
        assert!(says(harness, words), "the sheet says `{words}`");
    }
    assert!(has(harness, Role::Button, "Delete document"));
    assert!(has(harness, Role::Button, "Cancel"));
    testkit::save_png(harness, "app-library-delete-document");
}

fn cancel_escape_and_a_click_beside_the_sheet_close_it_and_send_nothing(
    harness: &mut Window,
    seen: &Seen,
) {
    let sheet_is_open = |harness: &Window| has(harness, Role::Button, "Delete document");
    click(harness, Role::Button, "Cancel");
    assert!(!sheet_is_open(harness), "Cancel closes the sheet");
    nothing_was_sent(seen);

    click_on_the_card(harness, Role::Button, &delete_document_button(SAMPLE_PAGES));
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(!sheet_is_open(harness), "Escape closes the sheet");
    nothing_was_sent(seen);

    click_on_the_card(harness, Role::Button, &delete_document_button(SAMPLE_PAGES));
    click(harness, Role::Tab, "Ask");
    assert!(
        !sheet_is_open(harness),
        "a click beside the sheet closes it"
    );
    assert_eq!(
        shared(harness).tab,
        Tab::Library,
        "the click only closed the sheet"
    );
    nothing_was_sent(seen);
}

/// Opens the document in the Source panel and comes back to the Library tab.
fn the_source_panel_shows(harness: &mut Window, doc: DocId) {
    harness.state_mut().push(Intent::OpenSource {
        doc,
        page: 1,
        piece: None,
    });
    testkit::settle(harness);
    assert_eq!(
        shared(harness).source.target.map(|target| target.doc),
        Some(doc)
    );
    press(harness, COMMAND, egui::Key::Num2);
}

/// The Source panel shows the document, and the Ingest tab holds a check of a PDF, so the confirm
/// has both to put right.
fn the_source_panel_and_a_check_hold_on_to_the_document(harness: &mut Window, doc: DocId) {
    the_source_panel_shows(harness, doc);
    let request = IngestRequest {
        pdf: "/books/x/chapter-3-greeks.pdf".into(),
        media: "Option Volatility and Pricing".to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 3,
            name: "Greeks".to_owned(),
        },
        tags: Vec::new(),
    };
    harness.state_mut().push(Intent::CheckIngest(request));
    testkit::settle(harness);
    assert!(matches!(shared(harness).ingest, IngestJob::Checked { .. }));
}

fn the_confirm_sends_one_delete_and_the_card_is_gone(
    harness: &mut Window,
    seen: &Seen,
    doc: DocId,
) {
    let others_before = documents_of(harness).len() - 1;
    let loads = loads_of_the_catalogue(seen);
    click_on_the_card(harness, Role::Button, &delete_document_button(SAMPLE_PAGES));
    click(harness, Role::Button, "Delete document");

    assert_eq!(sent_document_deletes(seen), [doc]);
    assert_eq!(sent_media_deletes(seen), Vec::<String>::new());
    assert_eq!(loads_of_the_catalogue(seen), loads + 1);
    assert!(!says(harness, SAMPLE_PAGES), "the card is gone");
    let after = documents_of(harness);
    assert!(after.iter().all(|document| document.id != doc));
    assert_eq!(after.len(), others_before, "no other document went");
    assert!(
        says(harness, "Chapter 2 · Black Scholes In Depth"),
        "a document of another media is still listed"
    );
    assert!(
        says(harness, "Option Volatility and Pricing"),
        "its media is still listed"
    );
    assert!(says(harness, "No document yet"));
    assert_eq!(shared(harness).source.target, None, "the source closed");
    assert_eq!(shared(harness).ingest, IngestJob::Idle, "the check is gone");
    assert_eq!(shared(harness).notices[0].kind, NoticeKind::Done);
    testkit::save_png(harness, "app-library-document-deleted");
}

#[test]
fn a_document_is_deleted_from_its_card_once_its_sheet_is_confirmed() {
    let (mut harness, seen) = open_switched("black-scholes", &Arc::default(), Ingests::Run);
    click(&mut harness, Role::Tab, "Library");
    let doc = the_document(&harness, SAMPLE_PAGES).id;
    every_document_has_one_delete_button(&harness);
    pressing_delete_opens_a_sheet_that_says_what_goes(&mut harness);
    cancel_escape_and_a_click_beside_the_sheet_close_it_and_send_nothing(&mut harness, &seen);
    the_source_panel_and_a_check_hold_on_to_the_document(&mut harness, doc);
    the_confirm_sends_one_delete_and_the_card_is_gone(&mut harness, &seen, doc);
}

fn every_titled_media_has_one_delete_button_and_the_ingest_card_has_none(harness: &mut Window) {
    let catalogue = the_catalogue(harness);
    let titles: Vec<&str> = catalogue
        .media
        .iter()
        .filter_map(|media| media.title.as_deref())
        .collect();
    assert!(!titles.is_empty());
    for title in titles {
        let name = delete_media_button(title);
        let buttons = harness.query_all_by_role_and_label(Role::Button, &name);
        assert_eq!(buttons.count(), 1, "`{name}` is one button");
    }

    click(harness, Role::Tab, "Ingest");
    click(harness, Role::ComboBox, "Media");
    click(harness, Role::Button, "Quanty Sample Notes · Book");
    assert!(
        has(harness, Role::Button, "Edit Quanty Sample Notes"),
        "the card of the chosen media is shown"
    );
    assert!(
        !says(harness, "Delete"),
        "the card of the Ingest tab has no Delete"
    );
    click(harness, Role::Tab, "Library");
}

fn pressing_delete_opens_a_sheet_that_counts_the_documents_that_go(harness: &mut Window) {
    click_on_the_card(harness, Role::Button, &delete_media_button(NOTES));
    assert!(
        has(
            harness,
            Role::Label,
            "Delete the media Quanty Sample Notes?"
        ),
        "the sheet names the media"
    );
    for words in [
        "2 documents",
        "passages, formulas, figures and tables of each",
        "converted pages of each",
        "cannot be undone",
    ] {
        assert!(says(harness, words), "the sheet says `{words}`");
    }
    assert!(has(harness, Role::Button, "Delete media"));
    assert!(has(harness, Role::Button, "Cancel"));
    testkit::save_png(harness, "app-library-delete-media");
}

fn the_confirm_sends_one_delete_and_the_media_goes_with_its_documents(
    harness: &mut Window,
    seen: &Seen,
) {
    let before = the_catalogue(harness);
    let documents_of_the_media: Vec<DocId> = before
        .documents_of_media(NOTES)
        .map(|document| document.id)
        .collect();
    assert_eq!(documents_of_the_media.len(), 2);
    let loads = loads_of_the_catalogue(seen);

    click(harness, Role::Button, "Delete media");

    assert_eq!(sent_media_deletes(seen), [NOTES]);
    assert_eq!(
        sent_document_deletes(seen),
        [],
        "the media is deleted by its title, not document by document"
    );
    assert_eq!(loads_of_the_catalogue(seen), loads + 1);
    let after = the_catalogue(harness);
    assert!(after.media_titled(NOTES).is_none(), "the media is gone");
    for doc in documents_of_the_media {
        assert!(after.document(doc).is_none(), "its documents are gone");
    }
    assert!(!says(harness, NOTES));
    assert_eq!(
        after.media_titled(BOOK),
        before.media_titled(BOOK),
        "another media is whole"
    );
    assert_eq!(shared(harness).source.target, None, "the source closed");
    assert_eq!(shared(harness).notices[0].kind, NoticeKind::Done);
}

fn a_media_with_no_document_is_deleted_too(harness: &mut Window, seen: &Seen) {
    let media = NewMedia {
        title: "Dynamic Hedging".to_owned(),
        ..NewMedia::default()
    };
    harness.state_mut().push(Intent::SaveMedia(media));
    testkit::settle(harness);
    click_on_the_card(
        harness,
        Role::Button,
        &delete_media_button("Dynamic Hedging"),
    );
    assert!(says(harness, "It has no document"));

    click(harness, Role::Button, "Delete media");

    assert_eq!(sent_media_deletes(seen), [NOTES, "Dynamic Hedging"]);
    assert!(!says(harness, "Dynamic Hedging"), "the card is gone");
}

#[test]
fn a_media_is_deleted_with_its_documents_once_its_sheet_is_confirmed() {
    let (mut harness, seen) = open_switched("black-scholes", &Arc::default(), Ingests::Run);
    click(&mut harness, Role::Tab, "Library");
    every_titled_media_has_one_delete_button_and_the_ingest_card_has_none(&mut harness);
    let in_the_media = the_catalogue(&harness)
        .documents_of_media(NOTES)
        .next()
        .map(|document| document.id)
        .expect("the media of the notes has a document");
    the_source_panel_shows(&mut harness, in_the_media);
    pressing_delete_opens_a_sheet_that_counts_the_documents_that_go(&mut harness);
    the_confirm_sends_one_delete_and_the_media_goes_with_its_documents(&mut harness, &seen);
    a_media_with_no_document_is_deleted_too(&mut harness, &seen);
}

fn the_paper(harness: &Window) -> Document {
    the_document(harness, "Self-Exciting Order Flow")
}

fn a_refused_delete_closes_the_open_form_and_shows_its_hint_on_the_card(
    harness: &mut Window,
    seen: &Seen,
    refuses_changes: &AtomicBool,
) {
    let paper = the_paper(harness);
    let name = delete_document_button("Self-Exciting Order Flow");
    refuses_changes.store(true, Ordering::SeqCst);
    click(harness, Role::Button, "Edit tags");
    assert!(has(harness, Role::TextInput, "Tags"), "the form is open");
    let loads = loads_of_the_catalogue(seen);

    click_on_the_card(harness, Role::Button, &name);
    click(harness, Role::Button, "Delete document");

    assert_eq!(sent_document_deletes(seen), [paper.id]);
    assert!(
        !has(harness, Role::TextInput, "Tags"),
        "a confirmed delete closes the form, so its failure is not taken for a refused save"
    );
    assert!(says(harness, REFUSED_HINT), "the card says why");
    assert_eq!(the_paper(harness), paper, "the document is still listed");
    assert_eq!(loads_of_the_catalogue(seen), loads + 1);
    assert_eq!(shared(harness).notices[0].kind, NoticeKind::Failed);
    testkit::save_png(harness, "app-library-delete-refused");
}

fn the_same_delete_goes_through_and_takes_the_hint_with_the_card(
    harness: &mut Window,
    seen: &Seen,
    refuses_changes: &AtomicBool,
) {
    refuses_changes.store(false, Ordering::SeqCst);
    let name = delete_document_button("Self-Exciting Order Flow");
    click_on_the_card(harness, Role::Button, &name);
    click(harness, Role::Button, "Delete document");

    assert_eq!(sent_document_deletes(seen).len(), 2);
    assert!(!says(harness, REFUSED_HINT), "the hint is gone");
    assert!(
        !says(harness, "Self-Exciting Order Flow"),
        "the card is gone"
    );
}

fn a_refused_media_delete_shows_its_hint_on_its_card_and_the_library_is_read_again(
    harness: &mut Window,
    seen: &Seen,
    refuses_changes: &AtomicBool,
) {
    refuses_changes.store(true, Ordering::SeqCst);
    let loads = loads_of_the_catalogue(seen);
    click_on_the_card(harness, Role::Button, &delete_media_button(NOTES));
    click(harness, Role::Button, "Delete media");

    assert_eq!(sent_media_deletes(seen), [NOTES]);
    assert!(says(harness, REFUSED_HINT), "the card says why");
    assert_eq!(
        loads_of_the_catalogue(seen),
        loads + 1,
        "the library is read again, because documents before the one that failed are gone"
    );
    assert!(the_catalogue(harness).media_titled(NOTES).is_some());
}

/// The scene says that the library fails, so the fake fails every delete as the live backend does
/// when a store is down.
fn a_scene_whose_library_fails_fails_both_deletes() {
    let (mut harness, _seen) = open_switched("stores-down", &Arc::default(), Ingests::Run);
    let doc = DocId(Uuid::from_u128(1));
    harness.state_mut().push(Intent::DeleteDocument(doc));
    testkit::settle(&mut harness);
    harness
        .state_mut()
        .push(Intent::DeleteMedia(NOTES.to_owned()));
    testkit::settle(&mut harness);

    let library = &shared(&harness).library;
    let held = library.failures.get(&doc).map(|failure| failure.kind);
    assert_eq!(held, Some(FailureKind::QdrantDown));
    let MediaDelete::Failed { failure, .. } = &library.media_delete else {
        panic!(
            "the delete of the media did not fail: {:?}",
            library.media_delete
        );
    };
    assert_eq!(failure.kind, FailureKind::QdrantDown);
}

#[test]
fn a_delete_that_failed_shows_its_hint_on_its_card_and_the_library_is_read_again() {
    let refuses_changes = Arc::new(AtomicBool::new(false));
    let (mut harness, seen) = open_switched("black-scholes", &refuses_changes, Ingests::Run);
    click(&mut harness, Role::Tab, "Library");
    a_refused_delete_closes_the_open_form_and_shows_its_hint_on_the_card(
        &mut harness,
        &seen,
        &refuses_changes,
    );
    the_same_delete_goes_through_and_takes_the_hint_with_the_card(
        &mut harness,
        &seen,
        &refuses_changes,
    );
    a_refused_media_delete_shows_its_hint_on_its_card_and_the_library_is_read_again(
        &mut harness,
        &seen,
        &refuses_changes,
    );
    a_scene_whose_library_fails_fails_both_deletes();
}
