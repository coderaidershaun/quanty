//! The Library tab lists the stored books and their chapters, copies the id of a chapter, opens it
//! to read, and says what no sample shows. A book with a chapter has the labels of its chapters.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::app::layout::DEFAULT_WINDOW;
use gui::backend::fake::Fake;
use gui::backend::{Handler, Reply};
use gui::contract::{
    Book, Catalogue, ChapterLabel, Command, DocId, Document, Event, Failure, FailureKind, Intent,
    ItemCounts, LabelEdit, NewBook, StartupFacts, Tab,
};
use gui::state::SourceTarget;
use gui::testkit;
use uuid::Uuid;

use super::recording::{self, Seen};
use super::{
    COMMAND, Window, click, copied_by, field, has, is_enabled, is_open_tab, node, press, retype,
    says, shared,
};

const REFUSED_HINT: &str = "Start Qdrant at http://localhost:6334, then try again.";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ingests {
    Run,
    NeverEnd,
}

/// Wraps the fake so that a test can make it refuse a save of labels, and make every ingest run
/// for ever.
struct Switched<H> {
    inner: H,
    refuses_saves: Arc<AtomicBool>,
    ingests: Ingests,
}

impl<H: Handler> Handler for Switched<H> {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::SetLabels { .. } if self.refuses_saves.load(Ordering::SeqCst) => {
                let failure = Failure::new(FailureKind::QdrantDown, "the test refuses the save")
                    .with_hint(REFUSED_HINT);
                for event in command.failed(&failure) {
                    reply.send(event);
                }
            }
            Command::Ingest { .. } if self.ingests == Ingests::NeverEnd => {}
            other => self.inner.serve(other, reply).await,
        }
    }
}

fn open_switched(scene: &str, refuses_saves: &Arc<AtomicBool>, ingests: Ingests) -> (Window, Seen) {
    let refuses_saves = Arc::clone(refuses_saves);
    let (mut harness, seen) =
        recording::open_with(scene, DEFAULT_WINDOW, move |fake: Fake| Switched {
            inner: fake,
            refuses_saves,
            ingests,
        });
    testkit::settle(&mut harness);
    (harness, seen)
}

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

/// A book saved with another author and another tag than its chapters carry, and a second
/// chapter that was not ingested whole.
fn dynamic_hedging() -> Catalogue {
    let chapter = |number: u32, tags: &[&str], ingested_items: Option<u64>| Document {
        id: DocId(Uuid::from_u128(u128::from(number))),
        title: format!("Dynamic Hedging, chapter {number}"),
        chapter: Some(ChapterLabel {
            number,
            name: format!("Part {number}"),
        }),
        author: Some("Nassim Nicholas Taleb".to_owned()),
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
        books: vec![Book {
            title: Some("Dynamic Hedging".to_owned()),
            author: Some("N. Taleb".to_owned()),
            tags: vec!["draft".to_owned()],
            chapters: vec![
                chapter(1, &["hedging", "options"], Some(22)),
                chapter(2, &["options"], None),
            ],
        }],
    }
}

fn the_page_says_which_chapter_is_not_whole_and_shows_what_the_chapters_carry(
    harness: &mut Window,
) {
    click(harness, Role::Tab, "Library");
    let not_whole = harness
        .query_all_by_label_contains("did not finish")
        .count();
    assert_eq!(
        not_whole, 1,
        "one chapter of the two was not ingested whole"
    );
    assert!(
        harness.query_all_by_label("draft").next().is_none(),
        "the saved tag is not shown once the book has a chapter"
    );
    assert!(
        !says(harness, "N. Taleb"),
        "the saved author is not shown once the book has a chapter"
    );
    testkit::save_png(harness, "app-library-not-whole");
}

fn the_book_list_of_ingest_offers_what_the_chapters_carry(harness: &mut Window) {
    click(harness, Role::Tab, "Ingest");
    click(harness, Role::ComboBox, "Book");
    click(harness, Role::Button, "Dynamic Hedging");
    let author = node(harness, Role::TextInput, "Author").value();
    assert_eq!(author.as_deref(), Some("Nassim Nicholas Taleb"));
    let tags = node(harness, Role::TextInput, "Tags").value();
    assert_eq!(
        tags.as_deref(),
        Some("options"),
        "the tags that every chapter has, and not the saved `draft`"
    );
}

fn documents_of(harness: &Window) -> Vec<Document> {
    let catalogue = shared(harness).library.catalogue.ready();
    let catalogue = catalogue.expect("the catalogue is loaded");
    catalogue.documents().cloned().collect()
}

fn the_tab_opens_the_page_and_the_page_lists_the_books_and_their_chapters(harness: &mut Window) {
    for name in ["Ask", "Library", "Ingest"] {
        assert!(has(harness, Role::Tab, name), "no tab is named `{name}`");
    }
    click(harness, Role::Tab, "Library");
    assert_eq!(shared(harness).tab, Tab::Library);
    assert!(is_open_tab(harness, "Library"));

    let catalogue = shared(harness).library.catalogue.ready().cloned();
    let catalogue = catalogue.expect("the catalogue is loaded");
    assert_eq!(catalogue.books.len(), 2, "the scene has two sample books");
    for book in &catalogue.books {
        let title = book.title.as_deref().expect("a sample book has a title");
        assert!(says(harness, title), "the book `{title}` is listed");
    }
    for document in catalogue.documents() {
        let chapter = document.chapter.as_ref().expect("a sample chapter");
        let heading = format!("Chapter {} · {}", chapter.number, chapter.name);
        assert!(says(harness, &heading), "`{heading}` is listed");
        let pages = format!(
            "{} pages",
            document.pages.expect("a sample chapter has pages")
        );
        assert!(says(harness, &pages), "`{pages}` is said");
        assert!(says(harness, &document.items.to_string()));
        let author = document.author.as_deref().unwrap_or("No author");
        assert!(says(harness, author), "the author `{author}` is said");
        for tag in &document.tags {
            assert!(has(harness, Role::Label, tag), "the tag `{tag}` is said");
        }
    }
    testkit::save_png(harness, "app-library-list");
}

fn a_saved_book_with_no_chapter_is_listed_and_says_so(harness: &mut Window) {
    let book = NewBook {
        title: "Dynamic Hedging".to_owned(),
        author: Some("Nassim Taleb".to_owned()),
        tags: vec!["hedging".to_owned()],
    };
    harness.state_mut().push(Intent::SaveBook(book));
    testkit::settle(harness);
    assert!(says(harness, "Dynamic Hedging"));
    assert!(says(harness, "Nassim Taleb"));
    assert!(says(harness, "No chapter yet"));
    testkit::save_png(harness, "app-library-saved-book");
}

fn copy_id_puts_the_document_id_on_the_clipboard(harness: &mut Window) {
    let first = documents_of(harness).remove(0);
    let texts = copied_by(harness, |harness| {
        node(harness, Role::Button, "Copy id").click();
    });
    assert_eq!(texts, [first.id.0.to_string()]);
}

fn read_opens_the_chapter_in_the_source_panel(harness: &mut Window, seen: &Seen) {
    let first = documents_of(harness).remove(0);
    let loads_of_first = |seen: &Seen| {
        seen.count(
            |command| matches!(command, Command::LoadPage { doc, page: 1, .. } if *doc == first.id),
        )
    };
    assert_eq!(loads_of_first(seen), 0);
    click(harness, Role::Button, "Read");
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

fn sent_edits(seen: &Seen) -> Vec<LabelEdit> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::SetLabels { edit, .. } => Some(edit),
            _ => None,
        })
        .collect()
}

fn a_refused_save_shows_its_hint_and_keeps_what_was_typed(
    harness: &mut Window,
    seen: &Seen,
    refuses_saves: &AtomicBool,
) {
    let before = documents_of(harness).remove(0);
    click(harness, Role::Button, "Edit labels");
    assert_eq!(field(harness, "Author").as_deref(), Some(""));
    assert_eq!(field(harness, "Tags").as_deref(), Some("book, volatility"));
    assert!(
        !is_enabled(harness, Role::Button, "Save"),
        "nothing was changed, so there is nothing to save"
    );
    testkit::save_png(harness, "app-library-edit");

    retype(harness, "Author", "Sheldon Natenberg");
    retype(harness, "Tags", "Volatility, greeks");
    refuses_saves.store(true, Ordering::SeqCst);
    click(harness, Role::Button, "Save");

    let wanted = LabelEdit {
        doc: before.id,
        author: Some("Sheldon Natenberg".to_owned()),
        add: vec!["greeks".to_owned()],
        remove: vec!["book".to_owned()],
    };
    assert_eq!(sent_edits(seen), [wanted]);
    assert!(says(harness, REFUSED_HINT), "the refusal says what to do");
    assert_eq!(
        field(harness, "Author").as_deref(),
        Some("Sheldon Natenberg")
    );
    assert_eq!(
        field(harness, "Tags").as_deref(),
        Some("Volatility, greeks")
    );
    assert_eq!(
        documents_of(harness).remove(0),
        before,
        "nothing was stored"
    );
    testkit::save_png(harness, "app-library-refused");
}

fn the_same_save_goes_through_and_the_new_labels_show_everywhere(
    harness: &mut Window,
    seen: &Seen,
    refuses_saves: &AtomicBool,
) {
    refuses_saves.store(false, Ordering::SeqCst);
    click(harness, Role::Button, "Save");
    assert_eq!(sent_edits(seen).len(), 2);
    assert!(
        !has(harness, Role::TextInput, "Author"),
        "the form is closed"
    );
    let first = documents_of(harness).remove(0);
    assert_eq!(first.author.as_deref(), Some("Sheldon Natenberg"));
    assert_eq!(first.tags, ["greeks", "volatility"]);
    assert!(says(harness, "Sheldon Natenberg"));
    for tag in ["greeks", "volatility"] {
        assert!(has(harness, Role::Label, tag), "the tag `{tag}` is listed");
    }
    assert!(
        harness.query_all_by_label("book").next().is_none(),
        "the tag `book` was taken off"
    );
    testkit::save_png(harness, "app-library-relabelled");

    click(harness, Role::Tab, "Ask");
    click(harness, Role::ComboBox, "Authors");
    assert!(has(harness, Role::Button, "Sheldon Natenberg"));
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);
    click(harness, Role::ComboBox, "Tags");
    assert!(has(harness, Role::CheckBox, "greeks"));
    press(harness, egui::Modifiers::NONE, egui::Key::Escape);

    click(harness, Role::Tab, "Ingest");
    click(harness, Role::ComboBox, "Book");
    click(harness, Role::Button, "Option Volatility and Pricing");
    assert_eq!(
        field(harness, "Author").as_deref(),
        Some("Sheldon Natenberg")
    );
}

fn labels_cannot_be_edited_while_an_ingest_runs() {
    let (mut harness, _) = open_switched("ingest-ready", &Arc::default(), Ingests::NeverEnd);
    // The ingest never ends, so the app is never idle again: `click` and `settle` would wait
    // for ever.
    node(&harness, Role::Button, "Start ingest").click();
    harness.run_ok();
    assert!(shared(&harness).ingest.is_running());
    node(&harness, Role::Tab, "Library").click();
    harness.run_ok();
    let buttons: Vec<_> = harness
        .query_all_by_role_and_label(Role::Button, "Edit labels")
        .collect();
    assert!(!buttons.is_empty());
    for button in buttons {
        assert!(button.accesskit_node().is_disabled());
    }
    assert!(says(&harness, "An ingest is running"));
    testkit::save_png(&mut harness, "app-library-ingest-running");
}

#[test]
fn the_library_lists_what_is_stored_and_a_chapter_is_copied_read_and_relabelled() {
    let refuses_saves = Arc::new(AtomicBool::new(false));
    let (mut harness, seen) = open_switched("black-scholes", &refuses_saves, Ingests::Run);
    the_tab_opens_the_page_and_the_page_lists_the_books_and_their_chapters(&mut harness);
    a_saved_book_with_no_chapter_is_listed_and_says_so(&mut harness);
    copy_id_puts_the_document_id_on_the_clipboard(&mut harness);
    read_opens_the_chapter_in_the_source_panel(&mut harness, &seen);
    a_refused_save_shows_its_hint_and_keeps_what_was_typed(&mut harness, &seen, &refuses_saves);
    the_same_save_goes_through_and_the_new_labels_show_everywhere(
        &mut harness,
        &seen,
        &refuses_saves,
    );
    labels_cannot_be_edited_while_an_ingest_runs();
}

#[test]
fn the_library_says_what_no_sample_shows() {
    let mut harness = open_on(Listing(dynamic_hedging()), DEFAULT_WINDOW);
    the_page_says_which_chapter_is_not_whole_and_shows_what_the_chapters_carry(&mut harness);
    the_book_list_of_ingest_offers_what_the_chapters_carry(&mut harness);
}
