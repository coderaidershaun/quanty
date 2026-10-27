//! The icons of the app: one name for each glyph of the icon font.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2};

use super::fonts::ICONS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon(&'static str);

macro_rules! icons {
    ($($name:ident = $glyph:ident),+ $(,)?) => {
        impl Icon {
            $(pub const $name: Icon = Icon(egui_phosphor::light::$glyph);)+

            /// Every icon with the name it has here, so a screen can show them all.
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
    INSIGHT = LIGHTBULB,
    TEXT = TEXT_ALIGN_LEFT,
    FORMULA = FUNCTION,
    FIGURE = IMAGE,
    TABLE = TABLE,
    CODE = CODE,
    MINUS = MINUS,
    PLUS = PLUS,
    FIT = ARROWS_HORIZONTAL,
    RESET = ARROW_COUNTER_CLOCKWISE,
    MAXIMISE = FRAME_CORNERS,
    RESTORE = CORNERS_IN,
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
    ANSWER = ARTICLE,
    PATH = PATH,
    FOLLOW_UP = CHAT_CIRCLE_TEXT,
    BROKEN_IMAGE = IMAGE_BROKEN,
    SUN = SUN,
    MOON = MOON,
}

impl Icon {
    pub const fn glyph(self) -> &'static str {
        self.0
    }

    pub fn font(size: f32) -> FontId {
        FontId::new(size, ICONS.clone())
    }

    /// Paints the glyph centred on a point, so every widget draws an icon the same way.
    pub fn paint(self, painter: &Painter, centre: Pos2, size: f32, tint: Color32) {
        painter.text(
            centre,
            Align2::CENTER_CENTER,
            self.0,
            Icon::font(size),
            tint,
        );
    }

    pub fn rich(self, ui: &egui::Ui, size: f32) -> egui::RichText {
        egui::RichText::new(self.0)
            .font(Icon::font(size))
            .color(super::palette(ui.ctx()).text_secondary)
    }
}
