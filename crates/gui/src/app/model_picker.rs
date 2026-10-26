//! The model and thinking choices of the tab on screen: Ask, Ingest and Datasets each keep their
//! own, and Library shows none. The two dropdowns stand in the top bar when it has the room, and
//! otherwise behind one button that opens them in a sheet. Saving takes effect on the next
//! operation.
use super::region::region;
use crate::panels::PanelCx;
use crate::theme::{TextRole, space};
use crate::widgets::{self, Button, ControlSize, Dropdown};
use choices::{MODEL_CHOICES, THINKING_LEVELS, WorkChoices, WorkKind};
use contract::{Intent, StartupFacts, Tab};
use eframe::egui::{self, InnerResponse};

const MODEL_WIDTH: f32 = 212.0;
const THINKING_WIDTH: f32 = 152.0;
/// The width of the room that the top bar keeps for the two dropdowns side by side.
pub(super) const DROPDOWNS_WIDTH: f32 = 394.0;
/// The width of the room that the top bar keeps for the button that opens the sheet.
pub(super) const MODEL_BUTTON_WIDTH: f32 = 80.0;
/// The width of what the sheet holds, so that it fits the smallest window with its margins.
const SHEET_WIDTH: f32 = 260.0;
const BUSY_HINT: &str = "Model changes are available when the current operation finishes.";

/// Where the top bar puts the model choices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Place {
    /// The two dropdowns, in this room.
    Inline(egui::Rect),
    /// One button, in this room, that opens the dropdowns in a sheet.
    Button(egui::Rect),
}

impl Place {
    pub(super) fn rect(self) -> egui::Rect {
        match self {
            Place::Inline(rect) | Place::Button(rect) => rect,
        }
    }
}

pub(super) struct ModelPicker {
    choices: WorkChoices,
    /// Whether the sheet of the button is open.
    is_open: bool,
}
impl ModelPicker {
    pub(super) fn new(facts: &StartupFacts) -> Self {
        Self {
            choices: facts.model_choices.clone(),
            is_open: false,
        }
    }

    pub(super) fn draw(
        &mut self,
        ui: &mut egui::Ui,
        place: Place,
        is_idle: bool,
        cx: &mut PanelCx<'_>,
    ) {
        let work = match cx.shared.tab {
            Tab::Ask => WorkKind::Ask,
            Tab::Ingest => WorkKind::Ingest,
            Tab::Datasets => WorkKind::Datasets,
            Tab::Library => {
                self.is_open = false;
                return;
            }
        };
        match place {
            Place::Inline(room) => {
                // A window that grows wide enough to hold the dropdowns closes the sheet.
                self.is_open = false;
                self.show_inline(ui, room, work, is_idle, cx);
            }
            Place::Button(room) => {
                self.show_button(ui, room);
                if self.is_open {
                    self.show_sheet(ui.ctx(), work, is_idle, cx);
                }
            }
        }
    }

    fn show_inline(
        &mut self,
        ui: &mut egui::Ui,
        room: egui::Rect,
        work: WorkKind,
        is_idle: bool,
        cx: &mut PanelCx<'_>,
    ) {
        let error = cx.shared.model_choice_errors.get(&work);
        let top = if error.is_some() { 4.0 } else { 8.0 };
        let bottom = if error.is_some() { 2.0 } else { 6.0 };
        let rect = egui::Rect::from_min_max(
            egui::pos2(room.left(), room.top() + top),
            egui::pos2(room.right(), room.bottom() - bottom),
        );
        region(ui, "model_picker", rect, |ui| {
            if error.is_some() {
                ui.spacing_mut().item_spacing.y = 2.0;
            }
            let InnerResponse { inner, response } = ui.add_enabled_ui(is_idle, |ui| {
                ui.horizontal(|ui| self.dropdowns(ui, work, ControlSize::Small))
                    .inner
            });
            response.on_disabled_hover_text(BUSY_HINT);
            cx.intents.extend(inner);
            if let Some(error) = error {
                ui.label(TextRole::Small.rich("The model choice could not be saved."))
                    .on_hover_text(&error.hint);
            }
        });
    }

    fn show_button(&mut self, ui: &mut egui::Ui, room: egui::Rect) {
        region(ui, "model_button", room, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let button = Button::secondary("Model")
                    .size(ControlSize::Medium)
                    .selected(self.is_open)
                    .accessible_name("Model choices");
                if ui.add(button).clicked() {
                    self.is_open = true;
                }
            });
        });
    }

    fn show_sheet(
        &mut self,
        ctx: &egui::Context,
        work: WorkKind,
        is_idle: bool,
        cx: &mut PanelCx<'_>,
    ) {
        let error = cx.shared.model_choice_errors.get(&work);
        let sheet = widgets::modal(ctx, egui::Id::new("model choices"), |ui| {
            ui.set_width(SHEET_WIDTH);
            ui.label(TextRole::Heading.rich("Model choices"));
            ui.add_space(space::SM);
            let changed = ui
                .add_enabled_ui(is_idle, |ui| self.dropdowns(ui, work, ControlSize::Medium))
                .inner;
            cx.intents.extend(changed);
            // A touch screen has no hover, so the sheet shows in plain view what the bar shows
            // on hover. The hint of a failed save already says that the choice was not saved.
            if let Some(error) = error {
                ui.label(TextRole::Small.rich(&error.hint));
            }
            if !is_idle {
                ui.label(TextRole::Small.rich(BUSY_HINT));
            }
            ui.add_space(space::SM);
            if ui
                .add(Button::secondary("Done").size(ControlSize::Medium))
                .clicked()
            {
                ui.close();
            }
        });
        if sheet.should_close() {
            self.is_open = false;
        }
    }

    /// The Model and the Thinking level dropdowns of `work`. A change is kept and returned as the
    /// intent that saves it.
    fn dropdowns(
        &mut self,
        ui: &mut egui::Ui,
        work: WorkKind,
        size: ControlSize,
    ) -> Option<Intent> {
        let before = self.choices.clone();
        let choice = self.choices.of_mut(work);
        let models: Vec<_> = MODEL_CHOICES.iter().map(|(_, label)| *label).collect();
        let selected = MODEL_CHOICES.iter().position(|(id, _)| *id == choice.model);
        if let Some(index) = Dropdown::new("Model", &models, MODEL_WIDTH)
            .selected(selected)
            .size(size)
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
        if let Some(index) = Dropdown::new("Thinking level", &labels, THINKING_WIDTH)
            .selected(selected)
            .size(size)
            .show(ui)
        {
            choice.thinking = levels[index].into();
        }
        (self.choices != before).then(|| Intent::ChooseModel {
            work,
            choice: self.choices.of(work).clone(),
        })
    }
}
