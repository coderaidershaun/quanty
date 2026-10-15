//! The always-visible model and thinking choices. Saving takes effect on the next operation.
use super::region::region;
use crate::contract::StartupFacts;
use crate::widgets::{ControlSize, Dropdown};
use eframe::egui;
use model_settings::{MODEL_CHOICES, MODEL_SETTINGS_FILE, ModelSettings, THINKING_LEVELS};

pub(super) struct ModelPicker {
    settings: ModelSettings,
    path: Option<std::path::PathBuf>,
    error: Option<String>,
}
impl ModelPicker {
    pub fn new(facts: &StartupFacts) -> Self {
        let path = facts.home.join(MODEL_SETTINGS_FILE);
        let loaded = if facts.fixture.is_some() {
            Ok(ModelSettings::default())
        } else {
            ModelSettings::load_from(&path)
        };
        let error = loaded.as_ref().err().map(|error| error_message(error));
        Self {
            settings: loaded.unwrap_or_default(),
            path: facts.fixture.is_none().then_some(path),
            error,
        }
    }
    pub fn draw(&mut self, ui: &mut egui::Ui, bar: egui::Rect, is_idle: bool) {
        let rect = egui::Rect::from_min_max(
            egui::pos2(bar.right() - 410.0, bar.top() + 8.0),
            egui::pos2(bar.right() - 16.0, bar.bottom() - 6.0),
        );
        region(ui, "model_picker", rect, |ui| {
            let before = self.settings.clone();
            ui.add_enabled_ui(is_idle, |ui| {
                ui.horizontal(|ui| {
                    let models: Vec<_> = MODEL_CHOICES.iter().map(|(_, label)| *label).collect();
                    let selected = MODEL_CHOICES
                        .iter()
                        .position(|(id, _)| *id == self.settings.model);
                    if let Some(index) = Dropdown::new("Model", &models, 212.0)
                        .selected(selected)
                        .size(ControlSize::Small)
                        .show(ui)
                    {
                        self.settings.select(MODEL_CHOICES[index].0);
                    }
                    let levels: Vec<_> = THINKING_LEVELS
                        .iter()
                        .copied()
                        .filter(|level| *level != "ultra" || self.settings.supports_ultra())
                        .collect();
                    let labels: Vec<_> = levels
                        .iter()
                        .map(|level| format!("Thinking: {level}"))
                        .collect();
                    let selected = levels
                        .iter()
                        .position(|level| *level == self.settings.thinking);
                    if let Some(index) = Dropdown::new("Thinking level", &labels, 152.0)
                        .selected(selected)
                        .size(ControlSize::Small)
                        .show(ui)
                    {
                        self.settings.thinking = levels[index].into();
                    }
                    if let Some(error) = &self.error {
                        ui.label("⚠").on_hover_text(error);
                    }
                });
            })
            .response
            .on_disabled_hover_text(
                "Model changes are available when the current operation finishes.",
            );
            if self.settings != before
                && let Some(path) = &self.path
            {
                match self.settings.save_to(path) {
                    Ok(()) => self.error = None,
                    Err(error) => {
                        self.settings = before;
                        self.error = Some(error_message(&error));
                    }
                }
            }
        });
    }
}

fn error_message(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(error) = cause {
        message.push_str(": ");
        message.push_str(&error.to_string());
        cause = error.source();
    }
    message
}
