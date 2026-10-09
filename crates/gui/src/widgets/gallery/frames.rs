//! The frames that hold content, each in every state: the cards, and the header row that a
//! panel starts with.

use eframe::egui;

use super::{State, column, section, used};
use crate::theme::{Icon, Kind, TextRole, space};
use crate::widgets::look::Look;
use crate::widgets::{Button, Card, Stepper, section_header, separator};

/// The text of the stepper in the panel header, where the actions run from right to left.
const HEADER_STEP: &str = "2 of 6";

pub(super) fn cards(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Cards");
    let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
    ui.horizontal_top(|ui| {
        for (kind, tag, action) in [
            (
                Kind::Formula,
                "Formula (verbatim from source)",
                "Copy formula",
            ),
            (Kind::Figure, "Figure (from source)", "Open figure"),
        ] {
            column(ui, width, 0.0, |ui| {
                let card = Card::new().tag(kind, tag).action(Icon::COPY, action);
                let shown = card.show(ui, |ui| {
                    ui.label(TextRole::Body.rich(format!("A card for {kind:?} text.")));
                });
                if shown.action_clicked {
                    state.last_activated = Some("Card::action");
                }
            });
        }
    });
    for pair in [
        [
            ("Open result", None, false),
            ("Open result hovered", Some(Look::HOVERED), false),
        ],
        [
            ("Open result selected", None, true),
            ("Open result focused", Some(Look::FOCUSED), false),
        ],
    ] {
        ui.horizontal_top(|ui| {
            for (label, look, is_selected) in pair {
                column(ui, width, 0.0, |ui| {
                    let mut card = Card::new().clickable(label).selected(is_selected);
                    if let Some(look) = look {
                        card = card.preview(look);
                    }
                    let shown = card.show(ui, |ui| {
                        ui.label(TextRole::BodyStrong.rich(label));
                        ui.label(TextRole::Small.rich("The whole card is a button."));
                    });
                    used(state, &shown.response, "Card::clickable");
                });
            }
        });
    }
}

pub(super) fn panel_header(ui: &mut egui::Ui, state: &mut State) {
    section_header(ui, "A panel header", |ui| {
        ui.add(Button::icon_only(Icon::MAXIMISE, "Maximise"));
        let paged = Stepper::new(HEADER_STEP)
            .previous("Previous figure", true)
            .next("Next figure", true)
            .show(ui);
        if paged.is_some() {
            state.last_activated = Some("Stepper::header");
        }
    });
    separator(ui);
    ui.add_space(space::SM);
}
