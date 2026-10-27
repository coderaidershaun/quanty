//! Continues from an answer: suggested questions and a box for a new one. Each starts a new
//! ask.

mod body;
mod guidance;

use eframe::egui;

use crate::panels::{PanelCx, PanelWidth};
use crate::theme::{Icon, TextRole, space};
use crate::widgets;
use contract::{AskDraft, Intent};

/// A panel narrower than this gives its question box room for a finger on the send button.
const NARROW_BELOW: f32 = 380.0;

#[derive(Debug, Default)]
pub struct Local {
    seen: u64,
    draft: String,
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let shared = cx.shared;
    let ask = &shared.ask;
    if local.seen != ask.generation {
        *local = Local {
            seen: ask.generation,
            ..Local::default()
        };
    }
    let box_size = match PanelWidth::of(ui, NARROW_BELOW) {
        PanelWidth::Narrow => widgets::ControlSize::Large,
        PanelWidth::Roomy => widgets::ControlSize::Medium,
    };
    let mut asked = None;
    widgets::panel_frame(widgets::Surface::Desk).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.spacing_mut().item_spacing.y = space::SM;
        widgets::section_header(ui, guidance::TITLE, |ui| {
            ui.label(TextRole::Small.rich(ui, guidance::CAPTION));
        });
        let Some(guidance) = guidance::guidance(ask) else {
            widgets::Placeholder::empty(Icon::SEND, guidance::NOTHING_YET)
                .hint(guidance::NOTHING_YET_HINT)
                .show(ui);
            return;
        };
        let typed = ask_box(ui, &mut local.draft, box_size);
        let clicked = egui::ScrollArea::vertical()
            .id_salt(("follow_up", ask.generation))
            .auto_shrink([false, false])
            .show(ui, |ui| body::show(ui, &guidance))
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

fn ask_box(ui: &mut egui::Ui, draft: &mut String, size: widgets::ControlSize) -> Option<String> {
    let outcome = widgets::TextInput::new(guidance::BOX_LABEL, draft)
        .id_salt("follow_up")
        .placeholder(guidance::BOX_HINT)
        .trailing(Icon::SEND, guidance::SEND_LABEL)
        .size(size)
        .show(ui);
    outcome.response.on_hover_text(guidance::BOX_HOVER);
    let question = draft.trim();
    let asked = outcome.submitted || outcome.trailing_clicked;
    (asked && !question.is_empty()).then(|| question.to_owned())
}
