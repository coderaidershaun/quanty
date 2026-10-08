//! A PDF is checked as soon as it and its media are known, with no button, and a book's PDF waits
//! until its chapter is typed and no box of the PDF has the keyboard any more.

use std::path::PathBuf;

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{Category, DocumentName, IngestRequest, Intent};
use gui::testkit;

use super::{choose_in_the_media_list, is_preflight, last_checked, open};
use crate::flows::{Window, click, field, has, press, says, type_into};

const BOOK: &str = "Quanty Sample Notes";

fn pick(harness: &mut Window, file: &str) {
    harness
        .state_mut()
        .push(Intent::PdfPicked(PathBuf::from(file)));
    testkit::settle(harness);
}

#[test]
fn a_pdf_is_checked_by_itself_and_a_book_waits_for_its_chapter() {
    let (mut harness, seen) = open("idle", DEFAULT_WINDOW);
    click(&mut harness, Role::Tab, "Ingest");
    choose_in_the_media_list(&mut harness, &format!("{BOOK} · Book"));
    assert_eq!(
        seen.count(is_preflight),
        0,
        "a media with no PDF is not checked"
    );

    pick(&mut harness, "chapter-4-vega.pdf");

    assert_eq!(
        seen.count(is_preflight),
        1,
        "the pick is checked with no click"
    );
    assert_eq!(
        last_checked(&seen).name,
        DocumentName::Chapter {
            number: 4,
            name: "Vega".to_owned()
        }
    );
    assert!(!has(&harness, Role::Button, "Check the chapter"));
    assert!(says(
        &harness,
        "Chapter 4 · Vega — 12 pages. Not converted yet."
    ));

    pick(&mut harness, "ch3.pdf");

    assert_eq!(
        seen.count(is_preflight),
        1,
        "a book's PDF needs its chapter"
    );
    for name in ["Chapter number", "Chapter name"] {
        let typed = field(&harness, name).unwrap_or_default();
        assert_eq!(typed, "", "`{name}` belonged to the file before");
    }
    assert!(says(
        &harness,
        "Type the chapter number and the chapter name of this PDF."
    ));

    type_into(&mut harness, "Chapter number", "3");
    type_into(&mut harness, "Chapter name", "Greeks");
    testkit::settle(&mut harness);
    assert_eq!(
        seen.count(is_preflight),
        1,
        "nothing is checked while a box of the PDF has the keyboard"
    );

    press(&mut harness, egui::Modifiers::NONE, egui::Key::Enter);

    assert_eq!(seen.count(is_preflight), 2);
    let wanted = IngestRequest {
        pdf: PathBuf::from("ch3.pdf"),
        media: BOOK.to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 3,
            name: "Greeks".to_owned(),
        },
        tags: Vec::new(),
    };
    assert_eq!(last_checked(&seen), wanted);
}
