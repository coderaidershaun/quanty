//! The small marks that sit in text and beside it, each in every state: citation chips, kind
//! chips, badges, step markers, the legend dots and the logo.

use eframe::egui;

use super::{State, section, used};
use crate::theme::{Icon, Kind, TextRole, Tone};
use crate::widgets::look::Look;
use crate::widgets::{Badge, Chip, CitationChip, StepMarker, StepState, dot, logo};

/// A chip label that does not fit in its width, to show the cut.
const CUT_CHIP: &str = "A concept chip with a label far too long to fit";

pub(super) fn chips(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Chips, badges and markers");
    citation_chips(ui, state);
    kind_chips(ui, state);
    plain_chips(ui, state);
    badges(ui);
    markers(ui);
    logo(ui);
}

fn citation_chips(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal_wrapped(|ui| {
        for (number, look, is_selected) in [
            (1, None, false),
            (2, Some(Look::HOVERED), false),
            (3, None, true),
            (4, Some(Look::FOCUSED), false),
            (12, None, false),
        ] {
            let mut chip = CitationChip::new(number).selected(is_selected);
            if let Some(look) = look {
                chip = chip.preview(look);
            }
            used(state, &ui.add(chip), "CitationChip");
        }
    });
}

fn kind_chips(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal_wrapped(|ui| {
        let kinds = [
            (Kind::Concept, "Black-Scholes model"),
            (Kind::RelatedConcept, "Risk-neutral measure"),
            (Kind::Text, "Section 3.2 text"),
            (Kind::Formula, "Formula (3.17)"),
            (Kind::Figure, "Figure 3.1"),
            (Kind::Table, "Table 3.2"),
        ];
        for (kind, label) in kinds {
            used(state, &ui.add(Chip::kind(kind, label)), "Chip::kind");
        }
        let hovered = Chip::kind(Kind::Concept, "Hovered concept").preview(Look::HOVERED);
        used(state, &ui.add(hovered), "Chip::kind");
        let selected = Chip::kind(Kind::Formula, "Selected formula").selected(true);
        used(state, &ui.add(selected), "Chip::kind");
        let cut = Chip::kind(Kind::Concept, CUT_CHIP).max_width(150.0);
        used(state, &ui.add(cut), "Chip::cut");
    });
}

fn plain_chips(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal_wrapped(|ui| {
        used(
            state,
            &ui.add(Chip::plain("Show the step-by-step derivation")),
            "Chip::plain",
        );
        let hovered = Chip::plain("Compare with the binomial model").preview(Look::HOVERED);
        used(state, &ui.add(hovered), "Chip::plain");
        used(
            state,
            &ui.add(Chip::plain("Selected follow-up").selected(true)),
            "Chip::plain",
        );
    });
}

fn badges(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (text, tone) in [
            ("Neutral", Tone::Neutral),
            ("Info", Tone::Blue),
            ("Related", Tone::Purple),
            ("Formula", Tone::Magenta),
            ("Ingested", Tone::Success),
            ("Pending", Tone::Warning),
            ("Failed", Tone::Danger),
        ] {
            ui.add(Badge::new(text).tone(tone));
        }
        ui.add(Badge::new("Copied").tone(Tone::Success).icon(Icon::CHECK));
    });
}

fn markers(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (number, status) in [
            (1, StepState::Done),
            (2, StepState::Done),
            (3, StepState::Active),
            (4, StepState::Pending),
        ] {
            ui.add(StepMarker::new(number).tone(Tone::Magenta).state(status));
        }
        ui.add(StepMarker::new(5).state(StepState::Active));
    });
    ui.horizontal(|ui| {
        for tone in Tone::ALL {
            dot(ui, tone);
            ui.label(TextRole::Small.rich(format!("{tone:?}")));
        }
    });
}
