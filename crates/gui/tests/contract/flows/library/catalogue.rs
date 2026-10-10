//! The Library tab lists each media with its documents, copies the id of a document, opens it to
//! read, has no Delete for the whole group of documents with no media, and says what no sample
//! shows.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::{Handler, Reply};
use gui::contract::{
    Catalogue, Category, ChapterLabel, Command, DocId, Document, Event, Failure, Intent,
    ItemCounts, Media, NewMedia, StartupFacts, Tab,
};
use gui::state::SourceTarget;
use gui::testkit;
use uuid::Uuid;

use super::{Ingests, delete_buttons, documents_of, open_switched};
use crate::flows::recording::Seen;
use crate::flows::{
    COMMAND, Window, click, copied_by, has, is_open_tab, node, press, says, shared,
};

/// Lists one catalogue that the test wrote by hand and refuses every other command.
struct Listing(Catalogue);

impl Handler for Listing {
    async fn serve(&self, command: Command, reply: Reply) {
        if let Command::LoadCatalogue { request } = command {
            reply.send(Event::Catalogue {
                request,
                result: Ok(self.0.clone()),
            });
            return;
        }
        let failure = Failure::not_built("this command of the test");
        for event in command.failed(&failure) {
            reply.send(event);
        }
    }
}

fn open_on(handler: impl Handler, size: [f32; 2]) -> Window {
    let facts = StartupFacts {
        home: testkit::samples_folder(),
        env_file: None,
        fixture: None,
        anthropic_api_key_set: false,
    };
    let mut harness = testkit::app_on(handler, facts, Vec::new(), size);
    testkit::settle(&mut harness);
    harness
}

/// A book whose second chapter was not ingested whole.
fn dynamic_hedging() -> Catalogue {
    let chapter = |number: u32, tags: &[&str], ingested_items: Option<u64>| Document {
        id: DocId(Uuid::from_u128(u128::from(number))),
        title: format!("Dynamic Hedging, chapter {number}"),
        chapter: Some(ChapterLabel {
            number,
            name: format!("Part {number}"),
        }),
        authors: vec!["Nassim Nicholas Taleb".to_owned()],
        media_tags: vec!["hedging".to_owned()],
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        pages: Some(12),
        items: ItemCounts {
            chunks: 18,
            formulas: 3,
            figures: 1,
            tables: 0,
        },
        ingested_items,
        folder: Some(PathBuf::from("dynamic-hedging")),
    };
    Catalogue {
        media: vec![Media {
            title: Some("Dynamic Hedging".to_owned()),
            category: Category::Book,
            authors: vec!["Nassim Nicholas Taleb".to_owned()],
            tags: vec!["hedging".to_owned()],
            documents: vec![
                chapter(1, &["options"], Some(22)),
                chapter(2, &["options"], None),
            ],
        }],
    }
}

/// The book, and after it a picture that belongs to no media.
fn book_and_a_picture_with_no_media() -> Catalogue {
    let mut catalogue = dynamic_hedging();
    catalogue.media.push(Media {
        title: None,
        documents: vec![Document {
            id: DocId(Uuid::from_u128(9)),
            title: "A picture on its own".to_owned(),
            ingested_items: Some(1),
            ..Document::default()
        }],
        ..Media::default()
    });
    catalogue
}

fn the_group_with_no_media_has_no_delete_but_each_of_its_documents_has(harness: &mut Window) {
    assert!(says(harness, "No media"));
    assert!(has(
        harness,
        Role::Button,
        "Delete document A picture on its own"
    ));
    let catalogue = shared(harness).library.catalogue.ready().cloned();
    let titled = catalogue
        .expect("the catalogue is loaded")
        .media
        .iter()
        .filter(|media| media.title.is_some())
        .count();
    let media_deletes = delete_buttons(harness)
        .iter()
        .filter(|button| {
            let name = button.accesskit_node().label().unwrap_or_default();
            name.starts_with("Delete media ")
        })
        .count();
    assert_eq!(
        media_deletes, titled,
        "a titled media has a Delete and the group with no media has none"
    );
}

fn the_page_says_which_chapter_is_not_whole(harness: &mut Window) {
    click(harness, Role::Tab, "Library");
    let not_whole = harness
        .query_all_by_label_contains("did not finish")
        .count();
    assert_eq!(
        not_whole, 1,
        "one chapter of the two was not ingested whole"
    );
    assert!(says(harness, "Chapter 2 · Part 2"));
    testkit::save_png(harness, "app-library-not-whole");
}

fn the_tab_opens_the_page_and_the_page_lists_the_media_and_their_documents(harness: &mut Window) {
    for name in ["Ask", "Library", "Ingest"] {
        assert!(has(harness, Role::Tab, name), "no tab is named `{name}`");
    }
    click(harness, Role::Tab, "Library");
    assert_eq!(shared(harness).tab, Tab::Library);
    assert!(is_open_tab(harness, "Library"));

    let catalogue = shared(harness).library.catalogue.ready().cloned();
    let catalogue = catalogue.expect("the catalogue is loaded");
    assert_eq!(
        catalogue.media.len(),
        3,
        "the scene has two sample books and a paper"
    );
    for media in &catalogue.media {
        let title = media.title.as_deref().expect("a sample media has a title");
        assert!(says(harness, title), "the media `{title}` is listed");
    }
    let labels_named = |name: &str| {
        harness
            .query_all_by_role_and_label(Role::Label, name)
            .count()
    };
    assert_eq!(labels_named("Paper"), 1, "the paper has a category chip");
    assert_eq!(labels_named("Book"), 2, "each book has a category chip");
    for authors in ["A. Author, B. Author", "Quanty Team"] {
        assert!(
            has(harness, Role::Label, authors),
            "the authors `{authors}` are said on their media"
        );
    }
    assert_eq!(
        labels_named("hawkes"),
        1,
        "the media tag of the paper is a badge on the paper, and its document does not repeat it"
    );
    for document in catalogue.documents() {
        let name = document.name().label();
        assert!(says(harness, &name), "`{name}` is listed");
        let pages = format!(
            "{} pages",
            document.pages.expect("a sample document has pages")
        );
        assert!(says(harness, &pages), "`{pages}` is said");
        assert!(says(harness, &document.items.to_string()));
        for tag in &document.tags {
            assert!(has(harness, Role::Label, tag), "the tag `{tag}` is said");
        }
    }
    testkit::save_png(harness, "app-library-list");

    click(harness, Role::Tab, "Ask");
    click(harness, Role::ComboBox, "Authors");
    for author in ["A. Author", "B. Author", "Quanty Team"] {
        let rows = harness
            .query_all_by_role_and_label(Role::Button, author)
            .count();
        assert_eq!(rows, 1, "`{author}` is a row of Authors once");
    }
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
    click(harness, Role::Tab, "Library");
}

fn a_saved_book_with_no_document_is_listed_and_says_so(harness: &mut Window) {
    let media = NewMedia {
        title: "Dynamic Hedging".to_owned(),
        category: Category::Book,
        authors: vec!["Nassim Taleb".to_owned()],
        tags: vec!["hedging".to_owned()],
    };
    harness.state_mut().push(Intent::SaveMedia(media));
    testkit::settle(harness);
    assert!(says(harness, "Dynamic Hedging"));
    let books = harness
        .query_all_by_role_and_label(Role::Label, "Book")
        .count();
    assert_eq!(books, 3, "the saved book has its category chip too");
    assert!(has(harness, Role::Label, "Nassim Taleb"));
    assert!(says(harness, "No document yet"));
    testkit::save_png(harness, "app-library-saved-book");
}

fn copy_id_puts_the_document_id_on_the_clipboard(harness: &mut Window) {
    let first = documents_of(harness).remove(0);
    let texts = copied_by(harness, |harness| {
        node(harness, Role::Button, "Copy id").click();
    });
    assert_eq!(texts, [first.id.0.to_string()]);
}

/// The first document that the source view can open: the paper of the samples has no folder.
fn first_readable(harness: &Window) -> Document {
    documents_of(harness)
        .into_iter()
        .find(|document| document.folder.is_some())
        .expect("a sample chapter has its folder")
}

fn read_opens_the_chapter_in_the_source_panel(harness: &mut Window, seen: &Seen) {
    let first = first_readable(harness);
    let loads_of_first = |seen: &Seen| {
        seen.count(
            |command| matches!(command, Command::LoadPage { doc, page: 1, .. } if *doc == first.id),
        )
    };
    assert_eq!(loads_of_first(seen), 0);
    let read = harness
        .query_all_by_role_and_label(Role::Button, "Read")
        .find(|button| !button.accesskit_node().is_disabled())
        .expect("a Read button is on");
    read.click();
    testkit::settle(harness);
    assert_eq!(shared(harness).tab, Tab::Ask);
    let target = SourceTarget {
        doc: first.id,
        page: 1,
        piece: None,
    };
    assert_eq!(shared(harness).source.target, Some(target));
    assert_eq!(loads_of_first(seen), 1);

    press(harness, COMMAND, egui::Key::Num2);
    assert_eq!(
        shared(harness).tab,
        Tab::Library,
        "⌘2 opens the Library tab"
    );
}

#[test]
fn the_library_lists_each_media_as_a_card_and_a_document_is_copied_and_read() {
    let (mut harness, seen) = open_switched("black-scholes", &Arc::default(), Ingests::Run);
    the_tab_opens_the_page_and_the_page_lists_the_media_and_their_documents(&mut harness);
    a_saved_book_with_no_document_is_listed_and_says_so(&mut harness);
    copy_id_puts_the_document_id_on_the_clipboard(&mut harness);
    read_opens_the_chapter_in_the_source_panel(&mut harness, &seen);
}

#[test]
fn the_library_says_what_no_sample_shows() {
    let mut harness = open_on(Listing(book_and_a_picture_with_no_media()), DEFAULT_WINDOW);
    the_page_says_which_chapter_is_not_whole(&mut harness);
    the_group_with_no_media_has_no_delete_but_each_of_its_documents_has(&mut harness);
}
