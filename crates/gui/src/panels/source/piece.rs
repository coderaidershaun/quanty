//! One piece of a page drawn by its kind, for the cards of the Source panel and of the Library, and
//! for the text of a page that has no picture, and the scrolling that brings a piece into view.

use eframe::egui;

use crate::media::Media;
use crate::media::code;
use crate::media::images;
use crate::media::math::{self, MathRef};
use crate::media::rich_text::{self, Clicked, RichText};
use crate::theme::{self, TextRole};
use crate::widgets::Card;
use contract::{Intent, PagePiece, PieceKind};

const MIN_FIGURE_HEIGHT: f32 = 120.0;
const MAX_FIGURE_HEIGHT: f32 = 320.0;

pub(in crate::panels) const fn kind_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Heading { .. } => "Heading",
        PieceKind::Text => "Text",
        PieceKind::Formula => "Formula",
        PieceKind::Figure => "Figure",
        PieceKind::Table => "Table",
        PieceKind::Footnote => "Footnote",
        PieceKind::Code => "Code",
    }
}

pub(in crate::panels) fn body(
    ui: &mut egui::Ui,
    piece: &PagePiece,
    media: &mut Media,
    intents: &mut Vec<Intent>,
) {
    let words = |role| RichText::new(&piece.text, role);
    let clicked = match piece.kind {
        PieceKind::Heading { .. } => after_marker(
            ui,
            media,
            piece.label.as_deref(),
            &words(TextRole::Subheading),
        ),
        PieceKind::Text => rich_text::show(ui, media, &words(TextRole::Body)),
        PieceKind::Footnote => {
            after_marker(ui, media, piece.label.as_deref(), &words(TextRole::Small))
        }
        PieceKind::Formula => {
            math::show(ui, media, &MathRef::block(&piece.text));
            note(ui, piece.name.as_deref());
            None
        }
        PieceKind::Figure => {
            let clicked = match &piece.image {
                Some(image) => {
                    let alt = piece.label.as_deref().unwrap_or("Figure");
                    images::show(ui, media, image, alt, figure_room(ui));
                    None
                }
                None => rich_text::show(ui, media, &words(TextRole::Small)),
            };
            note(ui, piece.caption.as_deref());
            clicked
        }
        PieceKind::Table => {
            let clicked = rich_text::table(ui, media, &piece.text, TextRole::Small);
            note(ui, piece.caption.as_deref());
            clicked
        }
        PieceKind::Code => {
            let clicked = code::show(ui, &piece.text, piece.language.as_deref());
            note(ui, piece.caption.as_deref());
            clicked
        }
    };
    forward(clicked, intents);
}

fn forward(clicked: Option<Clicked>, intents: &mut Vec<Intent>) {
    match clicked {
        Some(Clicked::CopyText(source)) => intents.push(Intent::CopyText(source)),
        Some(Clicked::Citation(number)) => intents.push(Intent::SelectResult(number)),
        None => {}
    }
}

/// The words after their marker, such as a heading number or a footnote mark. The marker is set
/// in the same role as the words so that it reads in the same face.
fn after_marker(
    ui: &mut egui::Ui,
    media: &mut Media,
    marker: Option<&str>,
    words: &RichText<'_>,
) -> Option<Clicked> {
    ui.horizontal_top(|ui| {
        if let Some(marker) = marker {
            let marker_color = theme::palette(ui.ctx()).text_secondary;
            ui.label(words.role.rich(ui, marker).color(marker_color));
        }
        rich_text::show(ui, media, words)
    })
    .inner
}

fn note(ui: &mut egui::Ui, line: Option<&str>) {
    if let Some(line) = line {
        ui.label(TextRole::Caption.rich(ui, line));
    }
}

/// The figure fits in the view together with its card around it and its caption under it.
fn figure_room(ui: &egui::Ui) -> egui::Vec2 {
    let caption = ui.spacing().item_spacing.y + TextRole::Caption.line_height();
    let around = Card::height_around_content(ui) + caption;
    let height = (ui.clip_rect().height() - around).clamp(MIN_FIGURE_HEIGHT, MAX_FIGURE_HEIGHT);
    egui::vec2(ui.available_width(), height)
}

pub(super) fn bring_into_view(block: &egui::Response) {
    block.scroll_to_me_animation(Some(egui::Align::Min), egui::style::ScrollAnimation::none());
}

/// While the person scrolls or presses in the room that is left, a piece that was to be brought
/// into view stays where the person put the list.
pub(super) fn is_scrolled_by_person(ui: &egui::Ui) -> bool {
    let room = ui.available_rect_before_wrap();
    ui.rect_contains_pointer(room)
        && ui.input(|input| {
            input.smooth_scroll_delta() != egui::Vec2::ZERO || input.pointer.any_down()
        })
}
