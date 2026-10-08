//! The first row of the bar: the question box, the mode, and the Ask or Stop button.

use eframe::egui;
use egui::text::{CCursor, CCursorRange};

use super::Local;
use super::layout::{Rects, slot};
use super::note::refusal;
use crate::contract::{AskDraft, AskMode, Intent};
use crate::panels::PanelCx;
use crate::theme::Icon;
use crate::widgets::{Button, ControlSize, Dropdown, TextInput};

const PLACEHOLDER: &str = "Ask a question about your books";
const MODES: [(&str, AskMode); 2] = [
    ("Answer", AskMode::Answer),
    ("Results only", AskMode::ResultsOnly),
];

impl Local {
    pub(super) fn show_question(&mut self, ui: &mut egui::Ui, rects: &Rects, cx: &mut PanelCx<'_>) {
        let takes_focus = cx.shared.cues.focus_ask_bar > self.seen_focus_cue;
        self.seen_focus_cue = cx.shared.cues.focus_ask_bar;
        let input = slot(ui, "question", rects.question, |ui| {
            TextInput::new("question", "Question", &mut self.draft.question)
                .placeholder(PLACEHOLDER)
                .icon(Icon::SEARCH)
                .size(ControlSize::Large)
                .show(ui)
        });
        // HAZARD: ask for the focus only after the box is drawn. A press of `/` arrives as a key
        // and as a typed character in the same frame, and a box that already has the focus
        // while it is drawn would type that `/` into the question.
        if takes_focus {
            input.response.request_focus();
            select_all(ui.ctx(), input.response.id, &self.draft.question);
            // Draw the frame again at once, so the caret shows without a repaint request.
            ui.ctx().request_discard("the question box takes the focus");
        }
        let refusal = refusal(&self.draft);
        if input.submitted {
            match refusal {
                // The box lets go of the focus on Enter, so ask for it back.
                Some(_) => input.response.request_focus(),
                None => self.ask(cx.intents),
            }
        }
        self.show_mode(ui, rects);
        self.show_ask_or_stop(ui, rects, refusal, cx);
    }

    fn show_ask_or_stop(
        &self,
        ui: &mut egui::Ui,
        rects: &Rects,
        refusal: Option<&'static str>,
        cx: &mut PanelCx<'_>,
    ) {
        let width = rects.ask.width();
        slot(ui, "ask", rects.ask, |ui| {
            if cx.shared.ask.is_running() {
                let stop = ui.add(
                    Button::secondary("Stop")
                        .icon(Icon::STOP)
                        .size(ControlSize::Large)
                        .min_width(width),
                );
                // The second click of a double click on Ask lands on Stop. It must not stop the
                // ask that the first click started.
                if stop.clicked() && !stop.double_clicked() && !stop.triple_clicked() {
                    cx.intents.push(Intent::CancelAsk);
                }
                return;
            }
            let button = Button::primary("Ask")
                .size(ControlSize::Large)
                .min_width(width);
            let ask = ui.add_enabled(refusal.is_none(), button);
            let ask = match refusal {
                Some(reason) => ask.on_disabled_hover_text(reason),
                None => ask,
            };
            if ask.clicked() {
                self.ask(cx.intents);
            }
        });
    }

    fn show_mode(&mut self, ui: &mut egui::Ui, rects: &Rects) {
        let labels = MODES.map(|(label, _)| label);
        let selected = MODES.iter().position(|(_, mode)| *mode == self.draft.mode);
        let chosen = slot(ui, "mode", rects.mode, |ui| {
            Dropdown::new("mode", "Mode", &labels)
                .selected(selected)
                .size(ControlSize::Large)
                .width(rects.mode.width())
                .show(ui)
        });
        if let Some(index) = chosen {
            self.draft.mode = MODES[index].1;
        }
    }

    fn ask(&self, intents: &mut Vec<Intent>) {
        intents.push(Intent::Ask(AskDraft {
            question: self.draft.question.trim().to_owned(),
            mode: self.draft.mode,
            filters: self.draft.filters.clone(),
        }));
    }
}

fn select_all(ctx: &egui::Context, id: egui::Id, text: &str) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let whole = CCursorRange::two(CCursor::new(0), CCursor::new(text.chars().count()));
    state.cursor.set_char_range(Some(whole));
    egui::TextEdit::store_state(ctx, id, state);
}
