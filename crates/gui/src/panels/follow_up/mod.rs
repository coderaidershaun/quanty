//! Continues from an answer: suggested questions and a box for a new one. Each starts a new
//! ask.

mod body;
mod guidance;
#[cfg(test)]
mod tests;

use eframe::egui;

use crate::contract::{AskDraft, Intent};
use crate::panels::PanelCx;
use crate::theme::{Icon, TextRole, color, space};
use crate::widgets;

/// What the panel keeps between frames: the ask it shows, and the question being typed.
#[derive(Debug, Default)]
pub struct Local {
    seen: u64,
    draft: String,
}

/// Draws the panel into `ui`, which is its whole rectangle. It pushes `Intent::Ask` only.
pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    let ask = &shared.ask;
    if local.seen != ask.generation {
        *local = Local {
            seen: ask.generation,
            ..Local::default()
        };
    }
    let mut asked = None;
    widgets::panel_frame().show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.spacing_mut().item_spacing.y = space::SM;
        widgets::section_header(ui, guidance::TITLE, |ui| {
            ui.label(
                TextRole::Small
                    .rich(guidance::CAPTION)
                    .color(color::TEXT_MUTED),
            );
        });
        let Some(guidance) = guidance::guidance(ask) else {
            widgets::Placeholder::empty(Icon::SEND, guidance::NOTHING_YET)
                .hint(guidance::NOTHING_YET_HINT)
                .show(ui);
            return;
        };
        let typed = ask_box(ui, &mut local.draft);
        let height = ui.available_height();
        let clicked = egui::ScrollArea::vertical()
            .id_salt(("follow_up", ask.generation))
            .auto_shrink([false, false])
            .show(ui, |ui| body::show(ui, &guidance, height))
            .inner;
        asked = typed.or_else(|| clicked.map(str::to_owned));
    });
    if let Some(question) = asked {
        cx.intents.push(Intent::Ask(AskDraft {
            question,
            mode: ask.mode,
            filters: ask.filters.clone(),
        }));
    }
}

/// The question that was typed, when the person asked for it with Enter or the button.
fn ask_box(ui: &mut egui::Ui, draft: &mut String) -> Option<String> {
    let outcome = widgets::TextInput::new("follow_up", guidance::BOX_LABEL, draft)
        .placeholder(guidance::BOX_HINT)
        .trailing(Icon::SEND, guidance::SEND_LABEL)
        .size(widgets::ControlSize::Medium)
        .show(ui);
    outcome.response.on_hover_text(guidance::BOX_HOVER);
    let question = draft.trim();
    let asked = outcome.submitted || outcome.trailing_clicked;
    (asked && !question.is_empty()).then(|| question.to_owned())
}
