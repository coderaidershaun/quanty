//! The pictures that are kept of the app at rest: for each one, the scene, the size of the window
//! and what is done to the app before it is drawn.

use eframe::egui::accesskit::Role;
use gui::app::layout::{DEFAULT_WINDOW, MIN_WINDOW, PHONE_WINDOW, TABLET_WINDOW};
use gui::theme::Scheme;

/// The row of a book on the shelf, and the row of its first chapter once the book is open.
pub(super) const BOOK_ROW: &str = "Option Volatility and Pricing";
pub(super) const CHAPTER_ROW: &str = "Chapter 1 · Sample Pages";
/// The row of the sample notes, and the row of their second chapter once they are open.
const NOTES_ROW: &str = "Quanty Sample Notes";
const BLACK_SCHOLES_ROW: &str = "Chapter 2 · Black Scholes In Depth";

pub(super) enum Start {
    AsItOpens,
    AfterAClickOnTheFigureCitation,
    AfterAClickOnShare,
    /// The save dialog is stood in by a path in a folder that does not exist, so the export
    /// fails the same way on every machine.
    AfterAnExportThatFails,
    AfterAClickOnStartIngest,
    AfterAStartThatNeverEnds,
    AfterANewMediaIsTyped,
    AfterAClickOnTheLibraryTab,
    /// The Library tab, a media row, one of its chapter rows, then a view of the chapter.
    AfterOpeningAViewOfAChapter {
        media: &'static str,
        chapter: &'static str,
        view: &'static str,
    },
    AfterAClickOnAMediaPencil,
    AfterAClickOnAMediaDelete,
    AfterAClickOnAChapterRow,
    AfterAClickOnMaximiseInTheConceptGraph,
    /// One click on the control of that role and name, which is the only one of its name.
    AfterAClickOn {
        role: Role,
        name: &'static str,
    },
    /// The scene never rests, so it is looked at once its catalogue has loaded and its bar has
    /// come to the stall.
    AtTheStallOfItsBuild,
}

pub(super) struct Row {
    pub(super) picture: &'static str,
    pub(super) scene: &'static str,
    pub(super) window: [f32; 2],
    pub(super) scheme: Scheme,
    pub(super) start: Start,
}

pub(super) static ROWS: &[Row] = &[
    Row {
        picture: "ask-idle",
        scene: "idle",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-answer",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-answer-min",
        scene: "black-scholes",
        window: MIN_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-share-menu",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnShare,
    },
    Row {
        picture: "ask-export-failed",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAnExportThatFails,
    },
    Row {
        picture: "ask-source",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnTheFigureCitation,
    },
    Row {
        picture: "ask-no-sources",
        scene: "no-sources",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-answer-failed",
        scene: "answer-failed",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-stores-down",
        scene: "stores-down",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-first-run",
        scene: "first-run",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "library",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnTheLibraryTab,
    },
    Row {
        picture: "ingest-ready",
        scene: "ingest-ready",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ingest-failed",
        scene: "ingest-failed",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnStartIngest,
    },
    Row {
        picture: "ingest-new-book",
        scene: "idle",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterANewMediaIsTyped,
    },
    Row {
        picture: "ingest-running",
        scene: "ingest-running",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAStartThatNeverEnds,
    },
    Row {
        picture: "library-edit-media",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnAMediaPencil,
    },
    Row {
        picture: "library-delete-media",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnAMediaDelete,
    },
    Row {
        picture: "library-document",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnAChapterRow,
    },
    Row {
        picture: "library-code",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterOpeningAViewOfAChapter {
            media: NOTES_ROW,
            chapter: BLACK_SCHOLES_ROW,
            view: "Code",
        },
    },
    Row {
        picture: "library-formulas",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterOpeningAViewOfAChapter {
            media: NOTES_ROW,
            chapter: BLACK_SCHOLES_ROW,
            view: "Formulas",
        },
    },
    Row {
        picture: "library-figures",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterOpeningAViewOfAChapter {
            media: BOOK_ROW,
            chapter: CHAPTER_ROW,
            view: "Figures",
        },
    },
    Row {
        picture: "library-tables",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterOpeningAViewOfAChapter {
            media: BOOK_ROW,
            chapter: CHAPTER_ROW,
            view: "Tables",
        },
    },
    Row {
        picture: "ask-graph-maximised",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnMaximiseInTheConceptGraph,
    },
    Row {
        picture: "datasets-ready",
        scene: "datasets-ready",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "datasets-running",
        scene: "datasets-running",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AtTheStallOfItsBuild,
    },
    Row {
        picture: "balances-unread",
        scene: "balances-unread",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "ask-out-of-credit",
        scene: "out-of-credit",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "library-insights",
        scene: "black-scholes",
        window: [1536.0, 2112.0],
        scheme: Scheme::Light,
        start: Start::AfterOpeningAViewOfAChapter {
            media: NOTES_ROW,
            chapter: BLACK_SCHOLES_ROW,
            view: "Insights",
        },
    },
    Row {
        picture: "library-insights-none",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterOpeningAViewOfAChapter {
            media: BOOK_ROW,
            chapter: CHAPTER_ROW,
            view: "Insights",
        },
    },
    Row {
        picture: "phone-ask-answer",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "phone-ask-source",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnTheFigureCitation,
    },
    Row {
        picture: "phone-ask-graph",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOn {
            role: Role::Tab,
            name: "Graph",
        },
    },
    Row {
        picture: "phone-ask-path",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOn {
            role: Role::Tab,
            name: "Path",
        },
    },
    Row {
        picture: "phone-ask-filters-sheet",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOn {
            role: Role::Button,
            name: "Filters",
        },
    },
    Row {
        picture: "phone-ask-model-sheet",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOn {
            role: Role::Button,
            name: "Model choices",
        },
    },
    Row {
        picture: "phone-library",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnTheLibraryTab,
    },
    Row {
        picture: "phone-library-document",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnAChapterRow,
    },
    Row {
        picture: "phone-ingest-ready",
        scene: "ingest-ready",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "phone-datasets-ready",
        scene: "datasets-ready",
        window: PHONE_WINDOW,
        scheme: Scheme::Light,
        start: Start::AsItOpens,
    },
    Row {
        picture: "tablet-ask-source",
        scene: "black-scholes",
        window: TABLET_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnTheFigureCitation,
    },
    Row {
        picture: "tablet-library-document",
        scene: "black-scholes",
        window: TABLET_WINDOW,
        scheme: Scheme::Light,
        start: Start::AfterAClickOnAChapterRow,
    },
    Row {
        picture: "dark-ask-answer",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Dark,
        start: Start::AsItOpens,
    },
    Row {
        picture: "dark-ask-source",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Dark,
        start: Start::AfterAClickOnTheFigureCitation,
    },
    Row {
        picture: "dark-library-document",
        scene: "black-scholes",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Dark,
        start: Start::AfterAClickOnAChapterRow,
    },
    Row {
        picture: "dark-ingest-ready",
        scene: "ingest-ready",
        window: DEFAULT_WINDOW,
        scheme: Scheme::Dark,
        start: Start::AsItOpens,
    },
    Row {
        picture: "dark-phone-ask-answer",
        scene: "black-scholes",
        window: PHONE_WINDOW,
        scheme: Scheme::Dark,
        start: Start::AsItOpens,
    },
    // The shortest window in which the whole gallery fits with no scroll.
    Row {
        picture: "dark-gallery",
        scene: "gallery",
        window: [1536.0, 3078.0],
        scheme: Scheme::Dark,
        start: Start::AsItOpens,
    },
];
