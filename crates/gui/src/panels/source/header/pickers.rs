//! The book and chapter pickers: the lists they choose from, and what they show while the library
//! is not read.

use eframe::egui;

use crate::contract::{ChapterLabel, DocId, Loadable, PageView};
use crate::state::Shared;
use crate::widgets::Dropdown;

/// The books of the library and the chapters of the open document's book, as texts to choose
/// from. It is worked out again only when the library, the open document or the shown page's
/// document changes.
#[derive(Debug, Default)]
pub(super) struct Lists {
    key: Option<(u64, Option<DocId>, Option<DocId>)>,
    books: Vec<String>,
    first_chapters: Vec<DocId>,
    chapters: Vec<String>,
    chapter_docs: Vec<DocId>,
    /// The places of the open document's book and chapter in the lists above.
    open_book: Option<usize>,
    open_chapter: Option<usize>,
    /// What the pickers show while the library is not read: the shown page's own labels.
    page_book: Option<String>,
    page_chapter: Option<String>,
}

impl Lists {
    pub(super) fn refresh(&mut self, shared: &Shared) {
        let open = shared.source.target.map(|target| target.doc);
        let shown = shared.source.page.ready();
        let key = (shared.library.revision, open, shown.map(|page| page.doc));
        if self.key == Some(key) {
            return;
        }
        *self = Lists {
            key: Some(key),
            page_book: shown.map(book_text),
            page_chapter: shown
                .and_then(|page| page.chapter.as_ref())
                .map(chapter_text),
            ..Lists::default()
        };
        let Some(catalogue) = shared.library.catalogue.ready() else {
            return;
        };
        let with_chapters = catalogue
            .books
            .iter()
            .filter_map(|book| book.chapters.first().map(|first| (book, first.id)));
        for (book, first) in with_chapters {
            let is_open = book.chapters.iter().any(|chapter| Some(chapter.id) == open);
            if is_open {
                self.open_book = Some(self.books.len());
                for chapter in &book.chapters {
                    if Some(chapter.id) == open {
                        self.open_chapter = Some(self.chapters.len());
                    }
                    self.chapters.push(
                        chapter
                            .chapter
                            .as_ref()
                            .map_or_else(|| chapter.title.clone(), chapter_text),
                    );
                    self.chapter_docs.push(chapter.id);
                }
            }
            self.books
                .push(book.title.clone().unwrap_or_else(|| "No book".to_owned()));
            self.first_chapters.push(first);
        }
    }

    /// The book picker and the chapter picker. A choice of a book opens its first chapter.
    pub(super) fn pickers<'a>(&'a self, shared: &'a Shared) -> [Picker<'a>; 2] {
        let catalogue = &shared.library.catalogue;
        let is_read = catalogue.ready().is_some();
        let why_off = match catalogue {
            Loadable::Ready(_) => None,
            Loadable::Failed(failure) => Some(failure.hint.as_str()),
            Loadable::Idle | Loadable::Loading => Some("Reading the library…"),
        };
        let (book, chapter) = if is_read {
            ("Choose a book", "Choose a chapter")
        } else {
            (
                self.page_book.as_deref().unwrap_or("Choose a book"),
                self.page_chapter.as_deref().unwrap_or("Choose a chapter"),
            )
        };
        [
            Picker {
                salt: "book",
                label: "Book",
                options: &self.books,
                opens: &self.first_chapters,
                selected: self.open_book,
                placeholder: book,
                is_enabled: is_read && !self.books.is_empty(),
                why_off,
            },
            Picker {
                salt: "chapter",
                label: "Chapter",
                options: &self.chapters,
                opens: &self.chapter_docs,
                selected: self.open_chapter,
                placeholder: chapter,
                is_enabled: is_read && !self.chapters.is_empty(),
                why_off,
            },
        ]
    }
}

fn book_text(page: &PageView) -> String {
    page.book.clone().unwrap_or_else(|| "No book".to_owned())
}

fn chapter_text(chapter: &ChapterLabel) -> String {
    format!("Chapter {}: {}", chapter.number, chapter.name)
}

/// One dropdown, with what it shows, what each choice opens, and whether it takes a choice.
#[derive(Debug)]
pub(super) struct Picker<'a> {
    salt: &'a str,
    label: &'a str,
    options: &'a [String],
    /// The document that each option opens.
    opens: &'a [DocId],
    selected: Option<usize>,
    placeholder: &'a str,
    is_enabled: bool,
    /// Said on hover while the picker is off.
    why_off: Option<&'a str>,
}

impl Picker<'_> {
    /// Draws the picker and returns the document that the person's choice opens. A choice that
    /// is the selection already opens nothing.
    pub(super) fn show(&self, ui: &mut egui::Ui, width: f32) -> Option<DocId> {
        let shown = ui.add_enabled_ui(self.is_enabled, |ui| {
            Dropdown::new(self.salt, self.label, self.options)
                .selected(self.selected)
                .placeholder(self.placeholder)
                .width(width)
                .show(ui)
        });
        if let Some(why_off) = self.why_off {
            shown.response.on_disabled_hover_text(why_off);
        }
        let chosen = shown.inner.filter(|place| Some(*place) != self.selected)?;
        self.opens.get(chosen).copied()
    }
}
