//! What the form does with the book of the chapter: the list of the library, a new book, and the
//! box that shows at once when the library has no book to offer.

use std::path::PathBuf;

use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::{choose_book, has_button, page, press, request, says, type_into};
use crate::contract::{Book, Catalogue, Document, FailureKind, IngestRequest, Intent, Loadable};
use crate::state::Shared;
use crate::testkit::{self, Host, sample};

const ADD_A_NEW_BOOK: &str = "Add a new book…";
const NATENBERG: &str = "Option Volatility and Pricing";

fn document(author: Option<&str>, tags: &[&str]) -> Document {
    Document {
        author: author.map(str::to_owned),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        ..Document::default()
    }
}

fn book(title: Option<&str>, chapters: Vec<Document>) -> Book {
    Book {
        title: title.map(str::to_owned),
        chapters,
    }
}

/// A book whose documents differ in author and tags, a book whose authors tie, a book with no
/// author, and documents that name no book.
fn library() -> Catalogue {
    Catalogue {
        books: vec![
            book(
                Some(NATENBERG),
                vec![
                    document(Some("Sheldon Natenberg"), &["options", "volatility"]),
                    document(Some("S. Natenberg"), &["volatility", "options", "greeks"]),
                    document(Some("Sheldon Natenberg"), &["volatility", "options"]),
                ],
            ),
            book(
                Some("Zeta Notes"),
                vec![document(Some("Mia"), &["a"]), document(Some("Ann"), &["b"])],
            ),
            book(Some("Plain Book"), vec![document(None, &[])]),
            book(None, vec![document(Some("Nobody"), &["loose"])]),
        ],
    }
}

fn with_catalogue(catalogue: Loadable<Catalogue>) -> Harness<'static, Host> {
    let mut shared = Shared::default();
    shared.library.catalogue = catalogue;
    let mut harness = page(shared);
    harness.run();
    harness
}

fn pick_the_file(harness: &mut Harness<'_, Host>) -> PathBuf {
    let pdf = request().pdf;
    let picked = Intent::PdfPicked(pdf.clone());
    harness
        .state_mut()
        .shared
        .apply_intent(picked, &mut Vec::new());
    harness.run();
    pdf
}

/// What the closed `Book` list shows: the chosen title, or its placeholder.
fn shown_in_list(harness: &Harness<'_, Host>) -> Option<String> {
    harness
        .get_by_role_and_label(Role::ComboBox, "Book")
        .value()
}

fn field(harness: &Harness<'_, Host>, name: &str) -> String {
    harness
        .get_by_role_and_label(Role::TextInput, name)
        .value()
        .unwrap_or_default()
}

fn has_box(harness: &Harness<'_, Host>) -> bool {
    harness
        .query_by_role_and_label(Role::TextInput, "Book title")
        .is_some()
}

fn has_list(harness: &Harness<'_, Host>) -> bool {
    harness
        .query_by_role_and_label(Role::ComboBox, "Book")
        .is_some()
}

#[test]
fn a_book_chosen_from_the_list_fills_the_author_and_the_tags_and_is_checked_by_its_title() {
    let mut harness = with_catalogue(Loadable::Ready(library()));
    let pdf = pick_the_file(&mut harness);
    assert!(has_list(&harness) && !has_box(&harness));
    assert_eq!(shown_in_list(&harness).as_deref(), Some("Choose a book"));

    choose_book(&mut harness, NATENBERG);
    assert_eq!(shown_in_list(&harness).as_deref(), Some(NATENBERG));
    assert_eq!(field(&harness, "Author"), "Sheldon Natenberg");
    assert_eq!(field(&harness, "Tags"), "options, volatility");
    testkit::save_png(&mut harness, "ingest-book-list");
    let wanted = IngestRequest {
        pdf,
        book: NATENBERG.to_owned(),
        author: Some("Sheldon Natenberg".to_owned()),
        tags: vec!["options".to_owned(), "volatility".to_owned()],
    };
    assert_eq!(
        press(&mut harness, "Check the chapter"),
        vec![Intent::CheckIngest(wanted)]
    );

    // The person may edit what the book filled in.
    type_into(&mut harness, "Author", " Jr.");
    assert_eq!(field(&harness, "Author"), "Sheldon Natenberg Jr.");

    // A tie of authors goes to the first by text, and tags that not every document carries
    // are left out.
    choose_book(&mut harness, "Zeta Notes");
    assert_eq!(field(&harness, "Author"), "Ann");
    assert_eq!(field(&harness, "Tags"), "");

    // A book with no author clears the field.
    choose_book(&mut harness, "Plain Book");
    assert_eq!(field(&harness, "Author"), "");

    // A book that leaves the library, as after a delete, stays as the title of a new book.
    let others = Catalogue {
        books: vec![book(Some("Zeta Notes"), vec![document(None, &[])])],
    };
    harness.state_mut().shared.library.catalogue = Loadable::Ready(others);
    harness.run();
    assert_eq!(field(&harness, "Book title"), "Plain Book");
    assert!(has_button(&harness, "Choose from the library"));
}

#[test]
fn a_new_book_is_typed_in_a_box_and_the_button_goes_back_to_the_list() {
    let mut harness = with_catalogue(Loadable::Ready(library()));
    let pdf = pick_the_file(&mut harness);
    choose_book(&mut harness, NATENBERG);

    choose_book(&mut harness, ADD_A_NEW_BOOK);
    assert!(has_box(&harness) && !has_list(&harness));
    assert!(has_button(&harness, "Choose from the library"));
    assert_eq!(field(&harness, "Book title"), "");
    assert_eq!(field(&harness, "Author"), "");
    assert_eq!(field(&harness, "Tags"), "");
    testkit::save_png(&mut harness, "ingest-book-new");

    type_into(&mut harness, "Book title", " A Fresh Book ");
    let wanted = IngestRequest {
        pdf,
        book: "A Fresh Book".to_owned(),
        author: None,
        tags: Vec::new(),
    };
    assert_eq!(
        press(&mut harness, "Check the chapter"),
        vec![Intent::CheckIngest(wanted)]
    );

    // The check was made for the new book, so going back to the list clears it.
    assert_eq!(
        press(&mut harness, "Choose from the library"),
        vec![Intent::ClearIngest]
    );
    assert!(has_list(&harness) && !has_box(&harness));
    assert_eq!(shown_in_list(&harness).as_deref(), Some("Choose a book"));
    assert!(press(&mut harness, "Check the chapter").is_empty());
}

#[test]
fn with_no_book_in_the_library_the_box_shows_at_once_with_the_reason() {
    let failure = sample::failure(FailureKind::QdrantDown);
    let untitled = Catalogue {
        books: vec![book(None, vec![document(None, &[])])],
    };
    let cases = [
        (Loadable::Idle, "The library is still loading.".to_owned()),
        (
            Loadable::Loading,
            "The library is still loading.".to_owned(),
        ),
        (Loadable::Failed(failure.clone()), failure.hint),
        (
            Loadable::Ready(Catalogue::default()),
            "No book is stored yet.".to_owned(),
        ),
        (
            Loadable::Ready(untitled),
            "No stored document names its book, so there is no list to choose from. Type the book's title."
                .to_owned(),
        ),
    ];
    for (catalogue, reason) in cases {
        let mut harness = with_catalogue(catalogue.clone());
        assert!(says(&harness, &reason), "{reason}");
        assert!(has_box(&harness) && !has_list(&harness), "{reason}");
        assert!(!has_button(&harness, "Choose from the library"), "{reason}");
        if catalogue == Loadable::Ready(Catalogue::default()) {
            testkit::save_png(&mut harness, "ingest-book-first-run");
        }
    }
}

#[test]
fn a_library_with_one_titled_book_and_documents_without_a_book_lists_the_titled_one() {
    let catalogue = Catalogue {
        books: vec![
            book(None, vec![document(Some("Nobody"), &[])]),
            book(Some("Only Book"), vec![document(None, &[])]),
        ],
    };
    let mut harness = with_catalogue(Loadable::Ready(catalogue));
    assert!(has_list(&harness) && !has_box(&harness));
    harness
        .get_by_role_and_label(Role::ComboBox, "Book")
        .click();
    harness.run();
    assert!(has_button(&harness, "Only Book"));
    assert!(has_button(&harness, ADD_A_NEW_BOOK));
    assert!(!says(&harness, "No stored document names its book"));
}
