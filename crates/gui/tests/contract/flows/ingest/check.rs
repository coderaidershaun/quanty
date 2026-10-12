//! A PDF is checked as soon as it and its media are known, with no button, and a book's PDF waits
//! until its chapter is typed and no box of the PDF has the keyboard any more. A book's PDF can
//! also be checked as a part of the book that is not a chapter, with a title.

use std::path::PathBuf;

use eframe::egui;
use eframe::egui::accesskit::Role;
use gui::app::layout::DEFAULT_WINDOW;
use gui::contract::{Category, DocumentName, IngestRequest, Intent};
use gui::testkit;

use super::recording::Seen;
use super::{choose_in_the_media_list, is_preflight, last_checked, open};
use crate::flows::{Window, click, field, has, press, retype, says, type_into};

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

    a_book_pdf_that_is_not_a_chapter_is_checked_as_a_part(&mut harness, &seen);
}

fn a_book_pdf_that_is_not_a_chapter_is_checked_as_a_part(harness: &mut Window, seen: &Seen) {
    let checked_before = seen.count(is_preflight);
    let part = |title: &str| IngestRequest {
        pdf: PathBuf::from("index.pdf"),
        media: BOOK.to_owned(),
        category: Category::Book,
        name: DocumentName::Title(title.to_owned()),
        tags: Vec::new(),
    };

    pick(harness, "index.pdf");

    assert_eq!(
        seen.count(is_preflight),
        checked_before,
        "a file that is not a chapter waits for its chapter, until the part is chosen"
    );
    assert!(has(harness, Role::Button, "Chapter"));
    assert!(has(harness, Role::Button, "Other part"));

    click(harness, Role::Button, "Other part");

    assert_eq!(
        field(harness, "Title").as_deref(),
        Some("Index"),
        "the title is made from the words of the file name"
    );
    assert!(!has(harness, Role::TextInput, "Chapter number"));
    assert_eq!(seen.count(is_preflight), checked_before + 1);
    assert_eq!(last_checked(seen), part("Index"));

    retype(harness, "Title", "Mathematics Appendix");
    press(harness, egui::Modifiers::NONE, egui::Key::Enter);

    assert_eq!(seen.count(is_preflight), checked_before + 2);
    assert_eq!(last_checked(seen), part("Mathematics Appendix"));

    pick(harness, "chapter-5-rho.pdf");

    assert_eq!(
        last_checked(seen),
        IngestRequest {
            pdf: PathBuf::from("chapter-5-rho.pdf"),
            name: DocumentName::Chapter {
                number: 5,
                name: "Rho".to_owned()
            },
            ..part("")
        },
        "a file named as a chapter goes back to the chapter"
    );
    assert_eq!(field(harness, "Chapter number").as_deref(), Some("5"));
}
