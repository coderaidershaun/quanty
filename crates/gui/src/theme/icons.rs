//! The icons of the app: one name for each glyph of the icon font.

use eframe::egui::{self, FontId};

use super::color;
use super::fonts::ICONS;

/// One icon of the icon font, drawn as a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon(&'static str);

macro_rules! icons {
    ($($name:ident = $glyph:ident),+ $(,)?) => {
        impl Icon {
            $(pub const $name: Icon = Icon(egui_phosphor::regular::$glyph);)+

            /// Every icon with its name, for the gallery.
            pub const ALL: &'static [(&'static str, Icon)] =
                &[$((stringify!($name), Icon::$name)),+];
        }
    };
}

icons! {
    SEARCH = MAGNIFYING_GLASS,
    LIBRARY = BOOKS,
    INGEST = UPLOAD_SIMPLE,
    BELL = BELL,
    HELP = QUESTION,
    CARET_DOWN = CARET_DOWN,
    CARET_LEFT = CARET_LEFT,
    CARET_RIGHT = CARET_RIGHT,
    CLOSE = X,
    STOP = STOP,
    SHARE = LINK_SIMPLE,
    COPY = COPY,
    OPEN = ARROW_SQUARE_OUT,
    CHECK = CHECK,
    CONCEPT = HEXAGON,
    TEXT = TEXT_ALIGN_LEFT,
    FORMULA = FUNCTION,
    FIGURE = IMAGE,
    TABLE = TABLE,
    MINUS = MINUS,
    PLUS = PLUS,
    FIT = FRAME_CORNERS,
    SEND = PAPER_PLANE_TILT,
    INFO = INFO,
    WARNING = WARNING,
    ERROR = WARNING_CIRCLE,
    SUCCESS = CHECK_CIRCLE,
    RETRY = ARROW_CLOCKWISE,
    TAG = TAG,
    EDIT = PENCIL_SIMPLE,
    DELETE = TRASH,
    BOOK = BOOK_OPEN,
    DOCUMENT = FILE_TEXT,
    AUTHOR = USER,
    PDF = FILE_PDF,
    FOLDER = FOLDER_OPEN,
    PENDING = CIRCLE,
    KEYBOARD = KEYBOARD,
    GRAPH = GRAPH,
    BROKEN_IMAGE = IMAGE_BROKEN,
}

impl Icon {
    /// An icon the list above lacks, from the icon font's own name for it.
    pub const fn from_phosphor(glyph: &'static str) -> Self {
        Icon(glyph)
    }

    /// One character of the icon font.
    pub const fn glyph(self) -> &'static str {
        self.0
    }

    /// The icon font at `size` points.
    pub fn font(size: f32) -> FontId {
        FontId::new(size, ICONS.clone())
    }

    /// For `ui.label`, in the secondary text colour.
    pub fn rich(self, size: f32) -> egui::RichText {
        egui::RichText::new(self.0)
            .font(Icon::font(size))
            .color(color::TEXT_SECONDARY)
    }
}
