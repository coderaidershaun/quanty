//! One piece of a page drawn by its kind, for the cards and for the text of a page that has no
//! picture, and the scrolling that brings a piece into view.

use eframe::egui;

use super::tabs::TabCx;
use crate::contract::{Intent, PagePiece, PieceKind};
use crate::media::images;
use crate::media::math::{self, MathRef};
use crate::media::rich_text::{self, Clicked, RichText};
use crate::theme::{TextRole, color, size, space, stroke};

const MIN_FIGURE_HEIGHT: f32 = 120.0;
const MAX_FIGURE_HEIGHT: f32 = 320.0;
/// What a card spends on its tag, its caption, the gaps between them, its margins and its border,
/// which the figure cannot have.
///
/// SMELL: this is a guess at how the card widget lays itself out. When the look of the card
/// changes, this number is wrong and nothing says so: a figure can then be a little too high
/// for the list.
const CARD_ROWS: f32 = size::CONTROL_SM
    + TextRole::Small.line_height()
    + 2.0 * space::MD
    + 2.0 * space::SM
    + 2.0 * stroke::BORDER;

pub(super) const fn kind_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Heading { .. } => "Heading",
        PieceKind::Text => "Text",
        PieceKind::Formula => "Formula",
        PieceKind::Figure => "Figure",
        PieceKind::Table => "Table",
        PieceKind::Footnote => "Footnote",
    }
}

pub(super) fn body(ui: &mut egui::Ui, piece: &PagePiece, cx: &mut TabCx<'_>) {
    let media = &mut *cx.media;
    let words = |role| RichText::new(&piece.text, role);
    let clicked = match piece.kind {
        PieceKind::Heading { rank } => {
            let role = if rank == 1 {
                TextRole::Heading
            } else {
                TextRole::BodyStrong
            };
            after_marker(ui, piece.label.as_deref(), |ui| {
                rich_text::show(ui, media, &words(role))
            })
        }
        PieceKind::Text => rich_text::show(ui, media, &words(TextRole::Body)),
        PieceKind::Footnote => after_marker(ui, piece.label.as_deref(), |ui| {
            rich_text::show(ui, media, &words(TextRole::Small))
        }),
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
    };
    forward(clicked, cx.intents);
}

fn forward(clicked: Option<Clicked>, intents: &mut Vec<Intent>) {
    match clicked {
        Some(Clicked::CopyText(source)) => intents.push(Intent::CopyText(source)),
        Some(Clicked::Citation(number)) => intents.push(Intent::SelectResult(number)),
        None => {}
    }
}

fn after_marker(
    ui: &mut egui::Ui,
    marker: Option<&str>,
    add_text: impl FnOnce(&mut egui::Ui) -> Option<Clicked>,
) -> Option<Clicked> {
    ui.horizontal_top(|ui| {
        if let Some(marker) = marker {
            ui.label(TextRole::Label.rich(marker).color(color::TEXT_SECONDARY));
        }
        add_text(ui)
    })
    .inner
}

fn note(ui: &mut egui::Ui, line: Option<&str>) {
    if let Some(line) = line {
        ui.label(TextRole::Small.rich(line));
    }
}

fn figure_room(ui: &egui::Ui) -> egui::Vec2 {
    let height = (ui.clip_rect().height() - CARD_ROWS).clamp(MIN_FIGURE_HEIGHT, MAX_FIGURE_HEIGHT);
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
