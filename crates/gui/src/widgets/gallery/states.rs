//! The widgets of the gallery, each in every state it can be in. A demo widget that is there to
//! be used writes its name to `last_activated`, and the gallery prints it as "Last used".

// SMELL: this file is close to the limit of 500 lines. The next widget that the gallery shows
// needs a file of its own beside this one.

use eframe::egui::{self, Response};

use super::{State, column, section};
use crate::theme::{Icon, Kind, TextRole, Tone, size, space};
use crate::widgets::look::Look;
use crate::widgets::{
    Badge, Button, Card, Chip, Choice, CitationChip, Confirm, ControlSize, Dropdown, Notice,
    Placeholder, Slider, StepMarker, StepState, Stepper, Tab, TabStrip, TextInput, dot,
    indeterminate_bar, logo, panel_frame, progress_bar, section_header, separator, spinner,
};

const MODES: [&str; 3] = ["Answer", "Search only", "Formulas"];
const MEDIA_TITLES: [&str; 2] = [
    "Options, Futures, and Other Derivatives",
    "A media whose title is far too long to fit in its dropdown",
];
/// A chip label that does not fit in its width, to show the cut.
pub(super) const CUT_CHIP: &str = "A concept chip with a label far too long to fit";
/// A stepper text wider than the room a short one gets, to show that the stepper grows.
pub(super) const LONG_STEP: &str = "page 5 of 7";
/// The text of the stepper in the panel header, where the actions run from right to left.
pub(super) const HEADER_STEP: &str = "2 of 6";
/// The width of the dropdown whose choice is too long for it, to show that it stays that wide.
pub(super) const MEDIA_WIDTH: f32 = 160.0;
const PLACEHOLDER_AREA: egui::Vec2 = egui::vec2(196.0, 170.0);

pub(super) fn show(ui: &mut egui::Ui, state: &mut State) {
    buttons(ui, state);
    inputs(ui, state);
    tabs(ui, state);
    cards(ui, state);
    chips(ui, state);
    notices(ui, state);
    placeholders(ui, state);
    progress(ui);
    panel_header(ui, state);
    confirm(ui, state);
}

fn used(state: &mut State, response: &Response, name: &'static str) {
    if response.clicked() {
        state.last_activated = Some(name);
    }
}

fn buttons(ui: &mut egui::Ui, state: &mut State) {
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

fn inputs(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Inputs");
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
    ui.horizontal_wrapped(|ui| {
        let mode = Dropdown::new("Mode", &MODES)
            .selected(state.demo.mode)
            .placeholder("Choose a mode")
            .show(ui);
        if mode.is_some() {
            state.demo.mode = mode;
            state.last_activated = Some("Dropdown");
        }
        let empty = Dropdown::new("Filter", &MODES)
            .placeholder("All media")
            .show(ui);
        if empty.is_some() {
            state.last_activated = Some("Dropdown::empty");
        }
        let media = Dropdown::new("Media", &MEDIA_TITLES)
            .selected(Some(1))
            .width(MEDIA_WIDTH)
            .show(ui);
        if media.is_some() {
            state.last_activated = Some("Dropdown::width");
        }
        let large = Dropdown::new("Large mode", &MODES)
            .selected(Some(0))
            .size(ControlSize::Large)
            .show(ui);
        if large.is_some() {
            state.last_activated = Some("Dropdown::large");
        }
    });
    ui.horizontal(|ui| {
        if let Some(value) = Slider::new("Zoom", state.demo.zoom, 0.25..=4.0).show(ui) {
            state.demo.zoom = value;
            state.last_activated = Some("Slider");
        }
        ui.label(TextRole::Label.rich(format!("{:.0}%", state.demo.zoom * 100.0)));
    });
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

fn tabs(ui: &mut egui::Ui, state: &mut State) {
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

fn cards(ui: &mut egui::Ui, state: &mut State) {
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

fn chips(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Chips, badges and markers");
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
    logo(ui);
}

fn notices(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Notices");
    let info = Notice::info("No sources found")
        .body(
            "I can't answer this question from the ingested documents. Try broadening the \
             question or adding more relevant documents.",
        )
        .dismissable()
        .show(ui);
    if info.dismissed {
        state.last_activated = Some("Notice::dismiss");
    }
    let warning = Notice::warning("Qdrant is not running")
        .body("Start it with docker compose up, then try again.")
        .action("Retry")
        .show(ui);
    if warning.action_clicked {
        state.last_activated = Some("Notice::action");
    }
    Notice::error("The answer could not be written")
        .body("Claude reached its usage limit. Try again after it resets.")
        .show(ui);
    Notice::success("PDF ingested").show(ui);
}

fn placeholders(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Placeholders");
    ui.horizontal_top(|ui| {
        let areas = [
            Placeholder::empty(Icon::SEARCH, "Nothing here yet")
                .hint("Ask a question to see the results.")
                .action("Ask a question"),
            Placeholder::loading("Loading the page").hint("This takes a moment."),
            Placeholder::error("The page could not be read")
                .hint("Check that the document's folder exists.")
                .action("Try again"),
        ];
        for placeholder in areas {
            let shown = panel_frame()
                .show(ui, |ui| {
                    ui.set_min_size(PLACEHOLDER_AREA);
                    ui.set_max_size(PLACEHOLDER_AREA);
                    placeholder.show(ui)
                })
                .inner;
            if shown.action_clicked {
                state.last_activated = Some("Placeholder::action");
            }
        }
    });
}

fn progress(ui: &mut egui::Ui) {
    section(ui, "Progress");
    ui.horizontal(|ui| {
        spinner(ui, "Working");
        ui.label(TextRole::Small.rich("Working"));
    });
    column(ui, 360.0, 0.0, |ui| {
        progress_bar(ui, "Reading the page", 0.4);
        progress_bar(ui, "Ingest finished", 1.0);
        indeterminate_bar(ui, "Checking the PDF");
    });
}

fn panel_header(ui: &mut egui::Ui, state: &mut State) {
    section_header(ui, "A panel header", |ui| {
        ui.add(Button::icon_only(Icon::FIT, "Fit to width"));
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

fn confirm(ui: &mut egui::Ui, state: &mut State) {
    section(ui, "Confirm");
    let open = ui.add(Button::secondary("Open the dialog"));
    if open.clicked() {
        state.demo.is_confirm_open = true;
    }
    if state.demo.is_confirm_open {
        let sheet = Confirm::new(egui::Id::new("gallery-confirm"), "Delete this document?")
            .body("Its pages are removed from the library. This cannot be undone.")
            .confirm_label("Delete document")
            .destructive()
            .show(ui.ctx());
        match sheet {
            Some(Choice::Confirmed) => {
                state.demo.is_confirm_open = false;
                state.last_activated = Some("Confirm::confirmed");
            }
            Some(Choice::Cancelled) => {
                state.demo.is_confirm_open = false;
                state.last_activated = Some("Confirm::cancelled");
            }
            None => {}
        }
    }
    ui.add_space(size::CONTROL_LG);
}
