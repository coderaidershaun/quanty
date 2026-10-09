//! The widgets a person acts on, each in every state: the buttons, the fields that take a value,
//! and the tabs.

use eframe::egui;

use super::{State, section, used};
use crate::theme::{Icon, TextRole, Tone};
use crate::widgets::look::Look;
use crate::widgets::{Button, ControlSize, Dropdown, Slider, Stepper, Tab, TabStrip, TextInput};

const MODES: [&str; 3] = ["Answer", "Search only", "Formulas"];
const MEDIA_TITLES: [&str; 2] = [
    "Options, Futures, and Other Derivatives",
    "A media whose title is far too long to fit in its dropdown",
];
/// A stepper text wider than the room a short one gets, to show that the stepper grows.
const LONG_STEP: &str = "page 5 of 7";
const DROPDOWN_WIDTH: f32 = 140.0;
/// The width of the dropdown whose choice is too long for it, to show that it stays that wide.
const MEDIA_WIDTH: f32 = 160.0;

pub(super) fn buttons(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Buttons");
    ui.horizontal_wrapped(|ui| {
        for (label, look) in [
            ("Ask", None),
            ("Ask hovered", Some(Look::HOVERED)),
            ("Ask pressed", Some(Look::PRESSED)),
            ("Ask focused", Some(Look::FOCUSED)),
        ] {
            let mut button = Button::primary(label).icon(Icon::SEARCH);
            if let Some(look) = look {
                button = button.preview(look);
            }
            used(state, &ui.add(button), "Button::primary");
        }
        used(
            state,
            &ui.add(Button::primary("Ask loading").loading(true)),
            "Button::primary",
        );
        let disabled = ui.add_enabled(false, Button::primary("Ask disabled"));
        used(state, &disabled, "Button::primary");
    });
    ui.horizontal_wrapped(|ui| {
        for (label, look) in [
            ("Secondary", None),
            ("Secondary hovered", Some(Look::HOVERED)),
            ("Secondary pressed", Some(Look::PRESSED)),
        ] {
            let mut button = Button::secondary(label).icon(Icon::RETRY);
            if let Some(look) = look {
                button = button.preview(look);
            }
            used(state, &ui.add(button), "Button::secondary");
        }
        let chosen = ui.add(Button::secondary("Secondary selected").selected(true));
        used(state, &chosen, "Button::secondary");
        let disabled = ui.add_enabled(false, Button::secondary("Secondary disabled"));
        used(state, &disabled, "Button::secondary");
        for (label, look) in [("Ghost", None), ("Ghost hovered", Some(Look::HOVERED))] {
            let mut button = Button::ghost(label);
            if let Some(look) = look {
                button = button.preview(look);
            }
            used(state, &ui.add(button), "Button::ghost");
        }
        used(state, &ui.add(Button::danger("Delete")), "Button::danger");
        let hovered = Button::danger("Delete hovered").preview(Look::HOVERED);
        used(state, &ui.add(hovered), "Button::danger");
    });
    ui.horizontal_wrapped(|ui| {
        used(
            state,
            &ui.add(Button::icon_only(Icon::COPY, "Copy")),
            "Button::icon_only",
        );
        let hovered = Button::icon_only(Icon::SHARE, "Share").preview(Look::HOVERED);
        used(state, &ui.add(hovered), "Button::icon_only");
        let chosen = Button::icon_only(Icon::GRAPH, "Graph").selected(true);
        used(state, &ui.add(chosen), "Button::icon_only");
        let large = Button::icon_only(Icon::OPEN, "Open").size(ControlSize::Large);
        used(state, &ui.add(large), "Button::icon_only");
        let wide = Button::secondary("Wide button").min_width(220.0);
        used(state, &ui.add(wide), "Button::secondary");
    });
}

pub(super) fn inputs(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Inputs");
    text_inputs(ui, state);
    dropdowns(ui, state);
    slider(ui, state);
    steppers(ui, state);
}

fn text_inputs(ui: &mut egui::Ui, state: &mut State) {
    let demo = &mut state.demo;
    let question = TextInput::new("Question", &mut demo.question)
        .placeholder("Ask about your library…")
        .icon(Icon::SEARCH)
        .trailing(Icon::CLOSE, "Clear question")
        .width(560.0)
        .show(ui);
    if question.submitted {
        state.last_activated = Some("TextInput");
    }
    if question.trailing_clicked {
        state.demo.question.clear();
        state.last_activated = Some("TextInput::trailing");
    }
    let follow = TextInput::new("Follow up", &mut state.demo.follow_up)
        .placeholder("Ask a follow-up question…")
        .trailing(Icon::SEND, "Send")
        .size(ControlSize::Small)
        .width(560.0)
        .show(ui);
    if follow.trailing_clicked {
        state.last_activated = Some("TextInput::send");
    }
}

fn dropdowns(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal_wrapped(|ui| {
        let mode = Dropdown::new("Mode", &MODES, DROPDOWN_WIDTH)
            .selected(state.demo.mode)
            .placeholder("Choose a mode")
            .show(ui);
        if mode.is_some() {
            state.demo.mode = mode;
            state.last_activated = Some("Dropdown");
        }
        let empty = Dropdown::new("Filter", &MODES, DROPDOWN_WIDTH)
            .placeholder("All media")
            .show(ui);
        if empty.is_some() {
            state.last_activated = Some("Dropdown::empty");
        }
        let media = Dropdown::new("Media", &MEDIA_TITLES, MEDIA_WIDTH)
            .selected(Some(1))
            .show(ui);
        if media.is_some() {
            state.last_activated = Some("Dropdown::width");
        }
        let large = Dropdown::new("Large mode", &MODES, DROPDOWN_WIDTH)
            .selected(Some(0))
            .size(ControlSize::Large)
            .show(ui);
        if large.is_some() {
            state.last_activated = Some("Dropdown::large");
        }
    });
}

fn slider(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal(|ui| {
        if let Some(value) = Slider::new("Zoom", state.demo.zoom, 0.25..=4.0).show(ui) {
            state.demo.zoom = value;
            state.last_activated = Some("Slider");
        }
        ui.label(TextRole::Label.rich(format!("{:.0}%", state.demo.zoom * 100.0)));
    });
}

fn steppers(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal_wrapped(|ui| {
        let both = Stepper::new("p. 64")
            .previous("Previous page", true)
            .next("Next page", true)
            .show(ui);
        if both.is_some() {
            state.last_activated = Some("Stepper");
        }
        let first = Stepper::new("p. 1")
            .previous("Previous page", false)
            .next("Next page", true)
            .show(ui);
        if first.is_some() {
            state.last_activated = Some("Stepper::first");
        }
        let long = Stepper::new(LONG_STEP)
            .previous("Previous step", true)
            .next("Next step", true)
            .show(ui);
        if long.is_some() {
            state.last_activated = Some("Stepper::long");
        }
    });
}

pub(super) fn tabs(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Tabs");
    let counted = [
        Tab::new("Answer"),
        Tab::new("Results").count(28),
        Tab::new("Formulas").count(5),
        Tab::new("Figures").count(6),
        Tab::new("Tables").count(0),
    ];
    if let Some(index) = TabStrip::new(&counted, state.demo.tab).show(ui) {
        state.demo.tab = index;
        state.last_activated = Some("TabStrip");
    }
    let top_bar = [
        Tab::new("Ask").icon(Icon::SEARCH),
        Tab::new("Library").icon(Icon::LIBRARY),
        Tab::new("Ingest").icon(Icon::INGEST),
    ];
    if let Some(index) = TabStrip::new(&top_bar, state.demo.top_tab)
        .tone(Tone::Magenta)
        .show(ui)
    {
        state.demo.top_tab = index;
        state.last_activated = Some("TabStrip::magenta");
    }
    let compact = [
        Tab::new("Page"),
        Tab::new("Pictures"),
        Tab::new("Equations"),
    ];
    if let Some(index) = TabStrip::new(&compact, state.demo.compact_tab)
        .compact()
        .show(ui)
    {
        state.demo.compact_tab = index;
        state.last_activated = Some("TabStrip::compact");
    }
    let hovered = [Tab::new("Alpha"), Tab::new("Beta"), Tab::new("Gamma")];
    TabStrip::new(&hovered, 0).preview(Look::HOVERED).show(ui);
}
