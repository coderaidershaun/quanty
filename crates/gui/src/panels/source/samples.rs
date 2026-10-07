//! Pages for the tests of this panel, read from the committed sample chapters and handed to the
//! shared state the way the app hands them, so every state a test draws is one the rules make.

use std::fs;
use std::path::Path;

use eframe::egui::accesskit::Role;
use eframe::egui::{Pos2, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::Value;

use super::Local;
use crate::contract::{
    Catalogue, ChapterLabel, DocId, Event, ImageRef, Intent, PageBox, PagePiece, PageView,
    PieceKind,
};
use crate::state::Shared;
use crate::testkit::{self, Host, sample};
use crate::theme::{space, stroke};

/// The outer size of the panel at the default window and at the smallest.
pub(super) const DEFAULT_SIZE: [f32; 2] = [504.0, 547.0];
pub(super) const SMALLEST_SIZE: [f32; 2] = [420.0, 300.0];

/// The pages the tests draw, each chosen for what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sample {
    /// Option Volatility page 7: Figure 24-12, low on the page.
    FigureLow,
    /// Option Volatility page 6: a figure, a formula and a table.
    EveryKind,
    /// Option Volatility page 5: a figure near the top, printed page 233.
    FigureTop,
    /// Option Volatility page 4: formulas (7.3) and (7.4).
    Formulas,
    /// Option Volatility page 3: two captioned tables.
    Tables,
    /// Sample Notes, chapter 2, page 3: four formulas and no page picture.
    Text,
}

impl Sample {
    /// The document, its folder inside the samples, and the page.
    fn place(self) -> (DocId, &'static str, u32) {
        const VOLATILITY: &str = "option-volatility-and-pricing/chapter-1";
        const NOTES: &str = "quanty-sample-notes/chapter-2";
        let volatility = |page| (doc(3), VOLATILITY, page);
        match self {
            Sample::FigureLow => volatility(7),
            Sample::EveryKind => volatility(6),
            Sample::FigureTop => volatility(5),
            Sample::Formulas => volatility(4),
            Sample::Tables => volatility(3),
            Sample::Text => (doc(2), NOTES, 3),
        }
    }

    pub(super) fn doc(self) -> DocId {
        self.place().0
    }

    pub(super) fn page_number(self) -> u32 {
        self.place().2
    }

    /// The last piece of this kind on the page.
    ///
    /// # Panics
    /// When the page holds no piece of this kind.
    pub(super) fn piece(self, kind: PieceKind) -> PagePiece {
        let found = page(self)
            .pieces
            .into_iter()
            .rev()
            .find(|piece| piece.kind == kind);
        found.unwrap_or_else(|| panic!("{self:?} holds no piece of the kind {kind:?}"))
    }
}

pub(super) fn doc(number: u128) -> DocId {
    DocId(uuid::Uuid::from_u128(number))
}

pub(super) fn catalogue() -> Catalogue {
    sample::catalogue()
}

/// The title of the book that holds the document.
///
/// # Panics
/// When the document is not in a titled book of the sample catalogue.
pub(super) fn book_title(doc: DocId) -> String {
    let book = catalogue().book_of(doc).and_then(|book| book.title.clone());
    book.unwrap_or_else(|| panic!("{doc:?} is not in a titled sample book"))
}

/// The first chapter of the book that holds the document: its id and what the chapter picker
/// calls it.
///
/// # Panics
/// When the document is not in a book of the sample catalogue, or its first chapter has no label.
pub(super) fn first_chapter(doc: DocId) -> (DocId, String) {
    let catalogue = catalogue();
    let book = catalogue
        .book_of(doc)
        .expect("the sample document is in a book");
    let first = book.chapters.first().expect("a sample book has a chapter");
    let label = first
        .chapter
        .as_ref()
        .expect("a sample chapter has a label");
    (first.id, chapter_text(label))
}

/// What the chapter picker calls a chapter, such as `Chapter 3: The Model`.
pub(super) fn chapter_text(label: &ChapterLabel) -> String {
    format!("Chapter {}: {}", label.number, label.name)
}

/// The place of the page in its chapter, as the pager says it, such as `page 5 of 7`.
pub(super) fn place_text(page: &PageView) -> String {
    format!("page {} of {}", page.page, page.page_count)
}

/// The printed page number, as the pager says it, such as `p. 233`.
///
/// # Panics
/// When the page has no printed number.
pub(super) fn printed_text(page: &PageView) -> String {
    let printed = page.printed_page.as_deref();
    format!(
        "p. {}",
        printed.expect("the sample page has a printed number")
    )
}

/// The name of the node that is the picture of this sample page on screen.
pub(super) fn picture_name(sample: Sample) -> String {
    format!("Picture of page {}", sample.page_number())
}

/// The page as the app's reader gives it, built from the sample chapter's files.
///
/// # Panics
/// When a sample file is missing or is not what this reader expects.
pub(super) fn page(sample: Sample) -> PageView {
    let (doc, chapter, number) = sample.place();
    let catalogue = catalogue();
    let document = catalogue.document(doc).expect("the sample document exists");
    let chapter_folder = testkit::samples_folder().join(chapter);
    let folder = chapter_folder.join(format!("page-num-{number}"));
    let json = read_json(&folder.join("page.json"));
    let picture = |at: u32| {
        let path = chapter_folder
            .join(format!("page-num-{at}"))
            .join("page.png");
        path.exists().then_some(ImageRef { path })
    };
    let page_count = document
        .pages
        .expect("the sample document has a page count");
    PageView {
        doc,
        page: number,
        book: catalogue.book_of(doc).and_then(|book| book.title.clone()),
        chapter: document.chapter.clone(),
        page_count,
        printed_page: text_of(&json["printed-page-number"]),
        image: picture(number),
        previous_image: number.checked_sub(1).filter(|at| *at > 0).and_then(picture),
        next_image: Some(number + 1)
            .filter(|at| *at <= page_count)
            .and_then(picture),
        pieces: json["pieces"]
            .as_array()
            .expect("page.json lists pieces")
            .iter()
            .map(|piece| piece_of(&folder, piece))
            .collect(),
    }
}

fn piece_of(folder: &Path, json: &Value) -> PagePiece {
    let kind = match json["kind"].as_str() {
        Some("heading") => PieceKind::Heading {
            rank: json["rank"]
                .as_u64()
                .and_then(|rank| u8::try_from(rank).ok())
                .unwrap_or(1),
        },
        Some("text") => PieceKind::Text,
        Some("formula") => PieceKind::Formula,
        Some("figure") => PieceKind::Figure,
        Some("table") => PieceKind::Table,
        Some("footnote") => PieceKind::Footnote,
        other => panic!("a sample piece has the kind {other:?}"),
    };
    let file = json["file"].as_str().expect("a piece names its file");
    let picture = &json["image"];
    let shows_figure = picture["shows"].as_str() == Some("figure");
    let label = match kind {
        PieceKind::Heading { .. } => &json["printed-number"],
        _ => &json["label"],
    };
    PagePiece {
        number: json["number"]
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .expect("a piece has a number"),
        kind,
        label: text_of(label),
        name: text_of(&json["name"]),
        caption: text_of(&json["caption"]),
        text: read_text(&folder.join(file)),
        image: picture["file"].as_str().map(|name| ImageRef {
            path: folder.join(name),
        }),
        cut: shows_figure.then(|| cut_of(&picture["cut"])),
    }
}

fn cut_of(json: &Value) -> PageBox {
    let side = |name: &str| {
        json[name]
            .as_u64()
            .and_then(|side| u16::try_from(side).ok())
            .unwrap_or_else(|| panic!("a cut has a {name}"))
    };
    PageBox {
        left: side("left"),
        top: side("top"),
        right: side("right"),
        bottom: side("bottom"),
    }
}

fn text_of(json: &Value) -> Option<String> {
    json.as_str().map(str::to_owned)
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
        .trim_end()
        .to_owned()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&read_text(path))
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

/// The state after the library was read and nothing was opened.
pub(super) fn library(catalogue: Catalogue) -> Shared {
    let mut shared = Shared::default();
    let mut effects = Vec::new();
    shared.apply_intent(Intent::RefreshCatalogue, &mut effects);
    let request = shared.library.pending.expect("the catalogue is loading");
    let reply = Event::Catalogue {
        request,
        result: Ok(catalogue),
    };
    shared.apply_event(reply, &mut effects);
    shared
}

/// The state after the library was read and the page was opened and has arrived.
pub(super) fn opened(sample: Sample, piece: Option<u32>) -> Shared {
    let mut shared = library(catalogue());
    let mut effects = Vec::new();
    let open = Intent::OpenSource {
        doc: sample.doc(),
        page: sample.page_number(),
        piece,
    };
    shared.apply_intent(open, &mut effects);
    land(&mut shared, sample);
    shared
}

/// Answers the page load that is waiting with this page and its concepts.
///
/// # Panics
/// When no page load is waiting.
pub(super) fn land(shared: &mut Shared, sample: Sample) {
    land_view(shared, page(sample));
}

/// Answers the page load that is waiting with this page, which a test may have changed, and the
/// concepts of the sample pages.
///
/// # Panics
/// When no page load is waiting.
pub(super) fn land_view(shared: &mut Shared, view: PageView) {
    let request = shared.source.pending.expect("a page load is waiting");
    let mut effects = Vec::new();
    let page = Event::Page {
        request,
        result: Ok(view),
    };
    shared.apply_event(page, &mut effects);
    let concepts = Event::PageConcepts {
        request,
        result: Ok(sample::page_concepts()),
    };
    shared.apply_event(concepts, &mut effects);
}

/// The panel in a test window of this outer size.
pub(super) fn panel(size: [f32; 2], shared: Shared) -> Harness<'static, Host> {
    let mut local = Local::default();
    testkit::panel(size, shared, move |ui, cx| super::show(ui, &mut local, cx))
}

/// Where a panel of this outer size is in its test window. The test kit puts a gutter round the
/// panel.
pub(super) fn panel_rect(size: [f32; 2]) -> Rect {
    Rect::from_min_size(Pos2::ZERO + Vec2::splat(space::LG), Vec2::from(size))
}

/// Where the content of a panel of this outer size is: the panel less the border and the margin
/// of its frame. What lies outside it is not seen.
pub(super) fn panel_room(size: [f32; 2]) -> Rect {
    panel_rect(size).shrink(space::LG + stroke::BORDER)
}

/// True when a node of the panel has this text in its name.
pub(super) fn shows(harness: &Harness<'static, Host>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

/// True when the picture of this sample page is on screen.
pub(super) fn has_picture_of(harness: &Harness<'static, Host>, sample: Sample) -> bool {
    let name = picture_name(sample);
    harness
        .query_by_role_and_label(Role::Image, &name)
        .is_some()
}

/// Lets every queued picture and formula finish, then draws what they made.
pub(super) fn see_pictures(harness: &mut Harness<'static, Host>) {
    harness.run_ok();
    harness.state_mut().media.run_pending();
    harness.run_ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scaffold for building the reader above: each sample page reads as the catalogue and the
    /// sample files say.
    #[test]
    fn the_six_sample_pages_read() {
        let all = [
            Sample::FigureLow,
            Sample::EveryKind,
            Sample::FigureTop,
            Sample::Formulas,
            Sample::Tables,
            Sample::Text,
        ];
        let catalogue = catalogue();
        for sample in all {
            let view = page(sample);
            let document = catalogue.document(sample.doc());
            let document = document.expect("the sample document is in the catalogue");
            assert_eq!(view.page, sample.page_number());
            assert_eq!(Some(view.page_count), document.pages);
            assert_eq!(view.chapter, document.chapter);
            assert_eq!(view.book, Some(book_title(sample.doc())));
            assert!(view.printed_page.is_some() && !view.pieces.is_empty());
            let has_picture = view.image.is_some();
            assert_eq!(has_picture, sample != Sample::Text);
            assert_eq!(view.previous_image.is_some(), has_picture && view.page > 1);
            assert_eq!(
                view.next_image.is_some(),
                has_picture && view.page < view.page_count
            );
        }
        let figure = Sample::FigureLow.piece(PieceKind::Figure);
        assert!(figure.label.is_some() && figure.cut.is_some());
        let shared = opened(Sample::Formulas, None);
        let shown = shared.source.page.ready().map(|view| view.page);
        assert_eq!(shown, Some(Sample::Formulas.page_number()));
        let concepts = shared.source.concepts.ready().map(Vec::len);
        assert_eq!(concepts, Some(sample::page_concepts().len()));
        assert!(shared.library.catalogue.ready().is_some());
    }
}
