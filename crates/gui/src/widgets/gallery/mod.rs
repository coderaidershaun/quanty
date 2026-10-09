//! One screen that shows the colours, the text styles and every widget in every state, so a
//! reviewer sees the whole kit at once.

mod controls;
mod frames;
mod marks;
mod messages;
mod tokens;

use eframe::egui::{self, Response};

use crate::theme::{TextRole, space};

const TOKENS_WIDTH: f32 = 680.0;

/// What the gallery keeps between frames. Whoever shows the gallery owns it.
#[derive(Debug, Default)]
pub struct State {
    pub last_activated: Option<&'static str>,
    demo: Demo,
}

/// The values the demo widgets hold, so a text box keeps what is typed and a tab stays chosen.
#[derive(Debug)]
struct Demo {
    question: String,
    follow_up: String,
    mode: Option<usize>,
    zoom: f32,
    tab: usize,
    top_tab: usize,
    compact_tab: usize,
    is_confirm_open: bool,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            question: String::new(),
            follow_up: String::new(),
            mode: None,
            zoom: 1.0,
            tab: 0,
            top_tab: 0,
            compact_tab: 0,
            is_confirm_open: false,
        }
    }
}

pub fn show(ui: &mut egui::Ui, state: &mut State) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.label(TextRole::Title.rich("Gallery"));
            ui.label(TextRole::Body.rich(format!(
                "Last used: {}",
                state.last_activated.unwrap_or("nothing yet")
            )));
            ui.horizontal_top(|ui| {
                let height = ui.available_height();
                column(ui, TOKENS_WIDTH, height, tokens::show);
                let width = ui.available_width();
                column(ui, width, height, |ui| widgets(ui, state));
            });
        });
}

/// The right column: every widget in every state. A demo widget that is there to be used writes
/// its name to `last_activated`, and the gallery prints it as "Last used".
fn widgets(ui: &mut egui::Ui, state: &mut State) {
    controls::buttons(ui, state);
    controls::inputs(ui, state);
    controls::tabs(ui, state);
    frames::cards(ui, state);
    marks::chips(ui, state);
    messages::notices(ui, state);
    messages::placeholders(ui, state);
    messages::progress(ui);
    frames::panel_header(ui, state);
    messages::confirm(ui, state);
}

fn used(state: &mut State, response: &Response, name: &'static str) {
    if response.clicked() {
        state.last_activated = Some(name);
    }
}

fn column(ui: &mut egui::Ui, width: f32, height: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    let layout = egui::Layout::top_down(egui::Align::Min);
    ui.allocate_ui_with_layout(egui::vec2(width, height), layout, add_contents);
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(space::LG);
    ui.label(TextRole::Heading.rich(title));
}

/// A wrapped row of nested layouts does not wrap by itself, so the rows are cut here.
fn rows<T>(ui: &mut egui::Ui, cell: f32, items: &[T], mut add_cell: impl FnMut(&mut egui::Ui, &T)) {
    let fit = (ui.available_width() / (cell + ui.spacing().item_spacing.x)).floor();
    let per_row = (fit as usize).max(1);
    for row in items.chunks(per_row) {
        ui.horizontal(|ui| {
            for item in row {
                add_cell(ui, item);
            }
        });
    }
}
