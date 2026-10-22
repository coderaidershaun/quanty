//! The always-visible model and thinking choices of the tab on screen: Ask, Ingest and Datasets
//! each keep their own, and Library shows none. Saving takes effect on the next operation.
use super::region::region;
use crate::widgets::{ControlSize, Dropdown};
use choices::{MODEL_CHOICES, THINKING_LEVELS, WorkChoices, WorkKind};
use contract::{StartupFacts, Tab};
use eframe::egui;

pub(super) struct ModelPicker {
    choices: WorkChoices,
}
impl ModelPicker {
    pub fn new(facts: &StartupFacts) -> Self {
        Self {
            choices: facts.model_choices.clone(),
        }
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        bar: egui::Rect,
        is_idle: bool,
        shared: &crate::state::Shared,
        intents: &mut Vec<contract::Intent>,
    ) {
        let work = match shared.tab {
            Tab::Ask => WorkKind::Ask,
            Tab::Ingest => WorkKind::Ingest,
            Tab::Datasets => WorkKind::Datasets,
            Tab::Library => return,
        };
        let error = shared.model_choice_errors.get(&work);
        let top = if error.is_some() { 4.0 } else { 8.0 };
        let bottom = if error.is_some() { 2.0 } else { 6.0 };
        let rect = egui::Rect::from_min_max(
            egui::pos2(bar.right() - 410.0, bar.top() + top),
            egui::pos2(bar.right() - 16.0, bar.bottom() - bottom),
        );
        region(ui, "model_picker", rect, |ui| {
            if error.is_some() {
                ui.spacing_mut().item_spacing.y = 2.0;
            }
            let before = self.choices.clone();
            let choice = self.choices.of_mut(work);
            ui.add_enabled_ui(is_idle, |ui| {
                ui.horizontal(|ui| {
                    let models: Vec<_> = MODEL_CHOICES.iter().map(|(_, label)| *label).collect();
                    let selected = MODEL_CHOICES.iter().position(|(id, _)| *id == choice.model);
                    if let Some(index) = Dropdown::new("Model", &models, 212.0)
                        .selected(selected)
                        .size(ControlSize::Small)
                        .show(ui)
                    {
                        choice.select(MODEL_CHOICES[index].0);
                    }
                    let levels: Vec<_> = THINKING_LEVELS
                        .iter()
                        .copied()
                        .filter(|level| *level != "ultra" || choice.supports_ultra())
                        .collect();
                    let labels: Vec<_> = levels
                        .iter()
                        .map(|level| format!("Thinking: {level}"))
                        .collect();
                    let selected = levels.iter().position(|level| *level == choice.thinking);
                    if let Some(index) = Dropdown::new("Thinking level", &labels, 152.0)
                        .selected(selected)
                        .size(ControlSize::Small)
                        .show(ui)
                    {
                        choice.thinking = levels[index].into();
                    }
                });
            })
            .response
            .on_disabled_hover_text(
                "Model changes are available when the current operation finishes.",
            );
            if self.choices != before {
                intents.push(contract::Intent::ChooseModel {
                    work,
                    choice: self.choices.of(work).clone(),
                });
            }
            if let Some(error) = error {
                ui.label(
                    crate::theme::TextRole::Small.rich("The model choice could not be saved."),
                )
                .on_hover_text(&error.hint);
            }
        });
    }
}
