//! The colours, sizes, text styles and icons that every panel draws with. It holds the final
//! value of every token; the fonts and the finished style are still to come.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Stroke};

use crate::contract::{ItemKind, NodeKind, PieceKind};

/// Sets the dark theme and the visuals that keep an idle window idle. Call it once, before the
/// first pass: the eframe creation closure.
pub fn install(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.visuals.panel_fill = color::CANVAS;
        style.visuals.text_cursor.blink = false;
        style.visuals.image_loading_spinners = false;
    });
}

/// `color::CANVAS`, for `eframe::App::clear_color`.
pub fn clear_color() -> [f32; 4] {
    color::CANVAS.to_normalized_gamma_f32()
}

/// A stroke one physical pixel wide.
pub fn hairline(painter: &egui::Painter, color: Color32) -> Stroke {
    Stroke::new(1.0 / painter.ctx().pixels_per_point(), color)
}

/// A soft halo in the tone's solid colour: `painter.add(glow(tone).as_shape(rect, radius))`.
pub fn glow(tone: Tone) -> egui::Shadow {
    egui::Shadow {
        offset: [0, 0],
        blur: 16,
        spread: 0,
        color: tone.swatch().solid.gamma_multiply(0.35),
    }
}

pub mod color {
    use super::Color32;

    pub const CANVAS: Color32 = Color32::from_rgb(0x07, 0x0E, 0x1A);
    pub const PANEL: Color32 = Color32::from_rgb(0x0A, 0x14, 0x21);
    pub const RAISED: Color32 = Color32::from_rgb(0x0E, 0x1C, 0x31);
    pub const RAISED_HOVER: Color32 = Color32::from_rgb(0x14, 0x27, 0x3F);
    pub const BORDER: Color32 = Color32::from_rgb(0x22, 0x3B, 0x5C);
    pub const HAIRLINE: Color32 = Color32::from_rgb(0x14, 0x26, 0x3C);
    pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xF4, 0xFB);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xB4, 0xC3, 0xD6);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x84, 0x96, 0xB0);
    pub const TEXT_ON_ACCENT: Color32 = Color32::from_rgb(0x0A, 0x06, 0x12);
    pub const PAGE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    pub const FOCUS: Color32 = Color32::from_rgb(0x7D, 0xB9, 0xFF);
    pub const SELECTION: Color32 = Color32::from_rgba_unmultiplied_const(0x0B, 0x7B, 0xF5, 0x66);
    pub const SCRIM: Color32 = Color32::from_rgba_unmultiplied_const(0x07, 0x0E, 0x1A, 0xB8);
    pub const PAGE_HIGHLIGHT: Color32 =
        Color32::from_rgba_unmultiplied_const(0x0B, 0x7B, 0xF5, 0x38);
}

/// Points.
pub mod space {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Points.
pub mod radius {
    pub const SM: f32 = 4.0;
    pub const MD: f32 = 6.0;
    pub const LG: f32 = 8.0;
    pub const XL: f32 = 12.0;
}

/// Points.
pub mod size {
    pub const CONTROL_SM: f32 = 28.0;
    pub const CONTROL_MD: f32 = 32.0;
    pub const CONTROL_LG: f32 = 40.0;
    pub const ICON_SM: f32 = 14.0;
    pub const ICON_MD: f32 = 16.0;
    pub const ICON_LG: f32 = 20.0;
    pub const CHIP: f32 = 24.0;
    pub const BADGE: f32 = 20.0;
    pub const CITATION: f32 = 18.0;
    pub const STEP: f32 = 24.0;
    pub const DOT: f32 = 8.0;
    pub const TAB: f32 = 44.0;
    pub const MATH_DISPLAY: f32 = 20.0;
}

/// Points.
pub mod stroke {
    pub const BORDER: f32 = 1.0;
    pub const FOCUS: f32 = 2.0;
    pub const UNDERLINE: f32 = 2.0;
    pub const EDGE: f32 = 1.5;
}

/// Seconds.
pub mod motion {
    pub const FAST: f32 = 0.12;
    pub const SPINNER_STEP: f32 = 0.1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    Neutral,
    Blue,
    Purple,
    Magenta,
    Success,
    Warning,
    Danger,
}

/// The five colours of one tone. `solid` is a fill or a stroke, `on_solid` is text on that
/// fill, `text` is the hue as readable text on a dark surface, `wash` is a dark tinted fill,
/// and `edge` is the outline of that fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swatch {
    pub solid: Color32,
    pub on_solid: Color32,
    pub text: Color32,
    pub wash: Color32,
    pub edge: Color32,
}

const fn swatch(solid: u32, on_solid: u32, text: u32, wash: u32, edge: u32) -> Swatch {
    const fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
    Swatch {
        solid: rgb(solid),
        on_solid: rgb(on_solid),
        text: rgb(text),
        wash: rgb(wash),
        edge: rgb(edge),
    }
}

impl Tone {
    pub const fn swatch(self) -> Swatch {
        match self {
            Tone::Neutral => swatch(0x8496B0, 0x0A0612, 0xB4C3D6, 0x0E1C31, 0x223B5C),
            Tone::Blue => swatch(0x0B7BF5, 0x0A0612, 0x4DA6FF, 0x05245A, 0x0F58B8),
            Tone::Purple => swatch(0x7A30F4, 0xEEF4FB, 0xA98BFF, 0x1D0C48, 0x6A2AD0),
            Tone::Magenta => swatch(0xF800C4, 0x0A0612, 0xFF4FD8, 0x3A053A, 0xC0089A),
            Tone::Success => swatch(0x22C58B, 0x0A0612, 0x4ADEA5, 0x062B22, 0x127A58),
            Tone::Warning => swatch(0xF2A93B, 0x0A0612, 0xF7C266, 0x33230A, 0x8F6217),
            Tone::Danger => swatch(0xF0503C, 0x0A0612, 0xFF8070, 0x3A120E, 0x9E2E22),
        }
    }
}

/// What a chip, a card tag, a graph node or a legend entry stands for. Each has one colour and
/// one icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Concept,
    RelatedConcept,
    Text,
    Formula,
    Figure,
    Table,
}

impl Kind {
    pub const fn tone(self) -> Tone {
        match self {
            Kind::Concept => Tone::Blue,
            Kind::RelatedConcept => Tone::Purple,
            Kind::Text => Tone::Neutral,
            Kind::Formula | Kind::Figure | Kind::Table => Tone::Magenta,
        }
    }

    pub const fn icon(self) -> Icon {
        match self {
            Kind::Concept | Kind::RelatedConcept => Icon::CONCEPT,
            Kind::Text => Icon::TEXT,
            Kind::Formula => Icon::FORMULA,
            Kind::Figure => Icon::FIGURE,
            Kind::Table => Icon::TABLE,
        }
    }
}

impl From<ItemKind> for Kind {
    fn from(kind: ItemKind) -> Kind {
        match kind {
            ItemKind::Chunk => Kind::Text,
            ItemKind::Formula => Kind::Formula,
            ItemKind::Figure => Kind::Figure,
            ItemKind::Table => Kind::Table,
        }
    }
}

impl From<NodeKind> for Kind {
    fn from(kind: NodeKind) -> Kind {
        match kind {
            NodeKind::Concept => Kind::Concept,
            NodeKind::Related => Kind::RelatedConcept,
            NodeKind::Formula => Kind::Formula,
            NodeKind::Figure => Kind::Figure,
            NodeKind::Table => Kind::Table,
        }
    }
}

impl From<PieceKind> for Kind {
    fn from(kind: PieceKind) -> Kind {
        match kind {
            PieceKind::Heading { .. } | PieceKind::Text | PieceKind::Footnote => Kind::Text,
            PieceKind::Formula => Kind::Formula,
            PieceKind::Figure => Kind::Figure,
            PieceKind::Table => Kind::Table,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Title,
    Heading,
    Body,
    BodyStrong,
    Label,
    Small,
    Micro,
    Mono,
}

impl TextRole {
    const fn size(self) -> f32 {
        match self {
            TextRole::Title => 20.0,
            TextRole::Heading => 16.0,
            TextRole::Body | TextRole::BodyStrong => 14.0,
            TextRole::Label | TextRole::Mono => 13.0,
            TextRole::Small => 12.0,
            TextRole::Micro => 11.0,
        }
    }

    const fn color(self) -> Color32 {
        match self {
            TextRole::Small => color::TEXT_SECONDARY,
            _ => color::TEXT,
        }
    }

    /// Size and weight, as one font.
    pub fn font(self) -> FontId {
        match self {
            TextRole::Mono => FontId::monospace(self.size()),
            _ => FontId::proportional(self.size()),
        }
    }

    /// Points.
    pub const fn line_height(self) -> f32 {
        match self {
            TextRole::Title => 28.0,
            TextRole::Heading | TextRole::Body | TextRole::BodyStrong => 22.0,
            TextRole::Label | TextRole::Mono => 18.0,
            TextRole::Small => 16.0,
            TextRole::Micro => 14.0,
        }
    }

    /// For a section of a layout job.
    pub fn format(self, color: Color32) -> egui::text::TextFormat {
        egui::text::TextFormat {
            font_id: self.font(),
            color,
            line_height: Some(self.line_height()),
            ..egui::text::TextFormat::default()
        }
    }

    /// For `ui.label`, in the colour of the role.
    pub fn rich(self, text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .font(self.font())
            .color(self.color())
    }

    /// One line of text, laid out and ready to paint.
    pub fn galley(self, ui: &egui::Ui, text: &str, color: Color32) -> Arc<egui::Galley> {
        ui.painter()
            .layout_no_wrap(text.to_owned(), self.font(), color)
    }

    /// The same size, in the semibold weight.
    pub fn strong_font(self) -> FontId {
        FontId::proportional(self.size())
    }

    /// Monospace, one point smaller.
    pub fn code_font(self) -> FontId {
        FontId::monospace(self.size() - 1.0)
    }

    /// The em of a formula set inline beside this role.
    pub const fn math_size(self) -> f32 {
        self.size() * 1.15
    }
}

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

    pub fn font(size: f32) -> FontId {
        FontId::proportional(size)
    }

    /// For `ui.label`, in the secondary text colour.
    pub fn rich(self, size: f32) -> egui::RichText {
        egui::RichText::new(self.0)
            .font(Icon::font(size))
            .color(color::TEXT_SECONDARY)
    }
}
