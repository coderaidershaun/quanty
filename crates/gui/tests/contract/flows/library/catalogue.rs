//! The Library tab lists each media on a shelf, shows the page of the open media or document,
//! copies the id of a document, opens it to read, has no Delete for the whole group of documents
//! with no media, and says what no sample shows.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::{Handler, Reply};
use gui::contract::{
    Catalogue, Category, ChapterLabel, Command, DocId, Document, DocumentView, Event, Failure,
    Intent, ItemCounts, Media, NewMedia, StartupFacts, Tab,
};
use gui::state::{Open, SourceTarget};
use gui::testkit;
use uuid::Uuid;

use super::{
    BLACK_SCHOLES, BOOK, HAWKES, Ingests, NOTES, SAMPLE_PAGES, documents_of, is_open_row,
    open_document, open_media, open_switched,
};
use crate::flows::recording::Seen;
use crate::flows::{
    COMMAND, Window, are_stacked_in_order, click, copied_by, has, is_open_tab, node, press, says,
    shared,
};

/// What the Insights view, which is not built yet, says under its title.
const VIEW_ON_ITS_WAY: &str = "This view comes in a later version of quanty.";

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
            code: 0,
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
    open_media(harness, "No media");
    assert!(has(harness, Role::Label, "No media"), "its page names it");
    let is_a_media_button = |harness: &Window| {
        harness.query_all_by_role(Role::Button).any(|button| {
            let name = button.accesskit_node().label().unwrap_or_default();
            name.starts_with("Edit ") || name.starts_with("Delete media ")
        })
    };
    assert!(
        !is_a_media_button(harness),
        "the group with no media has no pencil and no Delete"
    );
    open_document(harness, "No media", "A picture on its own");
    assert!(has(
        harness,
        Role::Button,
        "Delete document A picture on its own"
    ));

    open_media(harness, "Dynamic Hedging");
    assert!(
        has(harness, Role::Button, "Delete media Dynamic Hedging"),
        "a titled media has a Delete"
    );
}

fn the_page_says_which_chapter_is_not_whole(harness: &mut Window) {
    click(harness, Role::Tab, "Library");
    open_document(harness, "Dynamic Hedging", "Chapter 1 · Part 1");
    assert!(
        !says(harness, "did not finish"),
        "the first chapter is whole"
    );
    open_document(harness, "Dynamic Hedging", "Chapter 2 · Part 2");
    let not_whole = harness
        .query_all_by_label_contains("did not finish")
        .count();
    assert_eq!(not_whole, 1, "the second chapter was not ingested whole");
    assert!(has(harness, Role::Label, "Chapter 2 · Part 2"));
    testkit::save_png(harness, "app-library-not-whole");
}

fn labels_named(harness: &Window, name: &str) -> usize {
    harness
        .query_all_by_role_and_label(Role::Label, name)
        .count()
}

fn the_tab_opens_the_page_and_the_shelf_has_a_row_for_each_media(harness: &mut Window) {
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
    let titles: Vec<&str> = catalogue
        .media
        .iter()
        .map(|media| media.title.as_deref().expect("a sample media has a title"))
        .collect();
    assert!(
        are_stacked_in_order(harness, Role::Button, &titles),
        "the shelf lists the media in the order of the catalogue"
    );
    for media in &catalogue.media {
        let title = media.title.as_deref().expect("a sample media has a title");
        let count = node(harness, Role::Button, title).value();
        assert_eq!(
            count.as_deref(),
            Some(media.documents.len().to_string().as_str()),
            "the row of `{title}` counts its documents"
        );
    }
    assert!(
        is_open_row(harness, HAWKES),
        "the first media opens as soon as the library is read"
    );
    assert_eq!(
        shared(harness).shelf.open,
        Some(Open::Media {
            title: Some(HAWKES.to_owned())
        })
    );
    assert_eq!(labels_named(harness, "Paper"), 1, "its page has its badge");
    assert_eq!(
        labels_named(harness, "Book"),
        0,
        "no other media has a page"
    );
    assert!(has(harness, Role::Label, "A. Author, B. Author"));
    assert_eq!(
        labels_named(harness, "hawkes"),
        1,
        "the media tag of the paper is said once, on its page"
    );
    assert!(says(harness, "1 document"));
}

fn each_media_row_opens_its_page_and_unfolds_its_documents(harness: &mut Window) {
    let catalogue = shared(harness).library.catalogue.ready().cloned();
    let catalogue = catalogue.expect("the catalogue is loaded");
    for media in &catalogue.media {
        let title = media.title.as_deref().expect("a sample media has a title");
        open_media(harness, title);
        assert!(is_open_row(harness, title), "`{title}` is the open row");
        assert_eq!(labels_named(harness, media.category.label()), 1);
        if !media.authors.is_empty() {
            let line = media.authors.join(", ");
            assert!(has(harness, Role::Label, &line), "`{line}` is said");
        }
        let count = match media.documents.len() {
            1 => "1 document".to_owned(),
            count => format!("{count} documents"),
        };
        assert!(has(harness, Role::Label, &count), "`{count}` is said");
        for other in &catalogue.media {
            for document in &other.documents {
                let is_listed = has(harness, Role::Button, &document.name().label());
                assert_eq!(
                    is_listed,
                    other.title == media.title,
                    "only the open media unfolds its documents: `{title}` is open"
                );
            }
        }
    }
    assert!(
        are_stacked_in_order(
            harness,
            Role::Button,
            &[
                "Chapter 1 · Options Pricing Intuition",
                BLACK_SCHOLES,
                "Index",
            ]
        ),
        "the notes list their chapters first and then their index"
    );
    testkit::save_png(harness, "app-library-list");
}

fn each_document_row_opens_its_page(harness: &mut Window) {
    let catalogue = shared(harness).library.catalogue.ready().cloned();
    let catalogue = catalogue.expect("the catalogue is loaded");
    for media in &catalogue.media {
        let title = media.title.as_deref().expect("a sample media has a title");
        for document in &media.documents {
            let name = document.name().label();
            open_document(harness, title, &name);
            assert!(is_open_row(harness, &name), "`{name}` is the open row");
            assert_eq!(shared(harness).shelf.document(), Some(document.id));
            assert!(has(harness, Role::Label, &name), "its page names it");
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
    }
    testkit::save_png(harness, "app-library-document");

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

/// The Insights view, which is not built yet, says so with a placeholder and asks the backend for
/// nothing.
fn the_insights_view_shows_its_placeholder_and_asks_for_nothing(harness: &mut Window, seen: &Seen) {
    open_document(harness, BOOK, SAMPLE_PAGES);
    assert!(
        is_open_tab(harness, "Overview"),
        "the Overview is the first view"
    );
    let view = DocumentView::Insights;
    click(harness, Role::Tab, view.label());
    assert!(
        is_open_tab(harness, view.label()),
        "`{}` is shown",
        view.label()
    );
    assert_eq!(shared(harness).shelf.view, view);
    let placeholder = format!("{}. {VIEW_ON_ITS_WAY}", view.label());
    assert!(
        has(harness, Role::Label, &placeholder),
        "the view `{}` says that it comes later",
        view.label()
    );
    assert!(
        !says(harness, "not built"),
        "`{}` is not an error",
        view.label()
    );
    let asked = seen.count(|command| matches!(command, Command::LoadChapterInsights { .. }));
    assert_eq!(asked, 0, "the Insights view asks for nothing yet");

    open_document(harness, NOTES, BLACK_SCHOLES);
    assert!(
        is_open_tab(harness, DocumentView::Insights.label()),
        "the view that was shown stays when another document opens"
    );
    click(harness, Role::Tab, DocumentView::Overview.label());
    assert_eq!(shared(harness).shelf.view, DocumentView::Overview);
}

fn a_saved_book_with_no_document_is_a_row_with_none_and_says_so(harness: &mut Window) {
    let media = NewMedia {
        title: "Dynamic Hedging".to_owned(),
        category: Category::Book,
        authors: vec!["Nassim Taleb".to_owned()],
        tags: vec!["hedging".to_owned()],
    };
    harness.state_mut().push(Intent::SaveMedia(media));
    testkit::settle(harness);
    let row = node(harness, Role::Button, "Dynamic Hedging");
    assert_eq!(
        row.value().as_deref(),
        Some("0"),
        "its row counts no document"
    );
    open_media(harness, "Dynamic Hedging");
    assert_eq!(
        labels_named(harness, "Book"),
        1,
        "its page has its category badge"
    );
    assert!(has(harness, Role::Label, "Nassim Taleb"));
    assert!(says(harness, "No document yet"));
    testkit::save_png(harness, "app-library-saved-book");
}

/// The first document that the source view can open: the paper of the samples has no folder.
fn first_readable(harness: &Window) -> Document {
    documents_of(harness)
        .into_iter()
        .find(|document| document.folder.is_some())
        .expect("a sample chapter has its folder")
}

fn the_media_of(harness: &Window, document: &Document) -> String {
    let catalogue = shared(harness).library.catalogue.ready();
    let media = catalogue.and_then(|catalogue| catalogue.media_of(document.id));
    let title = media.and_then(|media| media.title.clone());
    title.expect("a sample document has a media with a title")
}

fn copy_id_puts_the_open_document_id_on_the_clipboard(harness: &mut Window, first: &Document) {
    open_document(
        harness,
        &the_media_of(harness, first),
        &first.name().label(),
    );
    let texts = copied_by(harness, |harness| {
        node(harness, Role::Button, "Copy id").click();
    });
    assert_eq!(texts, [first.id.0.to_string()]);
}

fn read_opens_the_chapter_in_the_source_panel(harness: &mut Window, seen: &Seen, first: &Document) {
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
    assert_eq!(
        shared(harness).shelf.open,
        Some(Open::Document(first.id)),
        "the Library shows what was open before"
    );
    assert!(is_open_row(harness, &first.name().label()));
}

#[test]
fn the_library_shows_each_media_on_the_shelf_and_each_document_on_its_page() {
    let (mut harness, seen) = open_switched("black-scholes", &Arc::default(), Ingests::Run);
    the_tab_opens_the_page_and_the_shelf_has_a_row_for_each_media(&mut harness);
    each_media_row_opens_its_page_and_unfolds_its_documents(&mut harness);
    each_document_row_opens_its_page(&mut harness);
    the_insights_view_shows_its_placeholder_and_asks_for_nothing(&mut harness, &seen);
    a_saved_book_with_no_document_is_a_row_with_none_and_says_so(&mut harness);
    let first = first_readable(&harness);
    copy_id_puts_the_open_document_id_on_the_clipboard(&mut harness, &first);
    read_opens_the_chapter_in_the_source_panel(&mut harness, &seen, &first);
}

#[test]
fn the_library_says_what_no_sample_shows() {
    let mut harness = open_on(Listing(book_and_a_picture_with_no_media()), DEFAULT_WINDOW);
    the_page_says_which_chapter_is_not_whole(&mut harness);
    the_group_with_no_media_has_no_delete_but_each_of_its_documents_has(&mut harness);
}
