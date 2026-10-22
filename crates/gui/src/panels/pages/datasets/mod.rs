//! The page that builds a training dataset from the converted documents: the form, then what the
//! one build of the app is doing.

mod form;
mod status;

use eframe::egui;

use crate::panels::PanelCx;
use crate::panels::seam::column;
use crate::state::Shared;
use crate::theme::{TextRole, space};
use contract::DatasetRequest;

/// The column is never wider than this, however wide the window.
const COLUMN_WIDTH: f32 = 720.0;
const TITLE: &str = "Build a training dataset";

#[derive(Debug, Default)]
pub struct Local {
    /// What the form holds. The name is as typed; `form::request_of` trims it.
    draft: DatasetRequest,
    choices: form::Choices,
    /// True once a person changed the form, or the form was filled from a build.
    is_touched: bool,
}

impl Local {
    /// The form reads the state and not an event, so a build that started while another tab was
    /// open, or before the window drew this page, is found when the page is drawn.
    fn follow(&mut self, shared: &Shared) {
        if !self.is_touched
            && let Some(request) = shared.datasets.request()
        {
            self.draft.clone_from(request);
            self.is_touched = true;
        }
        if self.choices.refresh(&shared.library) {
            self.choices.keep_known(&mut self.draft);
        }
    }
}

pub fn show(ui: &mut egui::Ui, local: &mut Local, cx: &mut PanelCx<'_>) {
    local.follow(cx.shared);
    column(ui, "datasets_column", COLUMN_WIDTH, |ui, height| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .max_height(height)
            .show(ui, |ui| {
                ui.label(TextRole::Title.rich(TITLE));
                ui.add_space(space::MD);
                let is_changed = ui
                    .add_enabled_ui(!cx.shared.datasets.is_running(), |ui| {
                        form::show(ui, &mut local.draft, &local.choices)
                    })
                    .inner;
                local.is_touched |= is_changed;
                ui.add_space(space::LG);
                status::show(ui, &local.draft, cx);
            });
    });
}
