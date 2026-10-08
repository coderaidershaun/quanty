//! How the results of the ask were found: the steps of the search, with what each produced.

mod row;
mod steps;

use std::mem::Discriminant;

use eframe::egui;

use crate::contract::{Loadable, SearchReply};
use crate::panels::PanelCx;
use crate::state::AskSession;
use crate::theme::space;
use crate::widgets;
use steps::{Body, Step};

#[derive(Debug, Default)]
pub struct Local {
    /// The ask, and the phase of its search, that `steps` were made for.
    made_for: Option<(u64, Discriminant<Loadable<SearchReply>>)>,
    steps: Vec<Step>,
}

impl Local {
    /// The rows change twice in one ask: waiting, then taken.
    fn follow(&mut self, ask: &AskSession) {
        let phase = (ask.generation, std::mem::discriminant(&ask.search));
        if self.made_for == Some(phase) {
            return;
        }
        self.made_for = Some(phase);
        self.steps = match &ask.search {
            Loadable::Loading => steps::waiting(),
            Loadable::Ready(reply) => steps::taken(&reply.trace),
            Loadable::Idle | Loadable::Failed(_) => Vec::new(),
        };
    }
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    let ask = &cx.shared.ask;
    local.follow(ask);
    widgets::panel_frame().show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.spacing_mut().item_spacing.y = space::SM;
        widgets::section_header(ui, steps::TITLE, |ui| {
            if ask.search.is_loading() {
                widgets::spinner(ui, steps::SEARCHING);
            }
        });
        match steps::body(ask) {
            Body::Steps => rows(ui, &local.steps),
            Body::Empty { icon, title, hint } => {
                widgets::Placeholder::empty(icon, title).hint(hint).show(ui);
            }
            Body::Failed(failure) => {
                widgets::Placeholder::error(steps::FAILED)
                    .hint(&failure.hint)
                    .show(ui)
                    .response
                    .on_hover_text(&failure.detail);
            }
        }
    });
}

fn rows(ui: &mut egui::Ui, steps: &[Step]) {
    let (pitch, density) = row::fit(ui.available_height(), steps.len());
    let width = ui.available_width();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut rects = Vec::with_capacity(steps.len());
        for (index, step) in steps.iter().enumerate() {
            let size = egui::vec2(width, pitch);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            row::show(ui, rect, index + 1, step, density);
            rects.push(rect);
        }
        for pair in rects.windows(2) {
            row::connect(ui, pair[0], pair[1]);
        }
    });
}
