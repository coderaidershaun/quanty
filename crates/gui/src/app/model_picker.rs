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

/// In the bar each dropdown is as wide as its longest choice, so that it stays small and its
/// width never changes with the choice.
const BAR_MODEL_WIDTH: f32 = 156.0;
const BAR_THINKING_WIDTH: f32 = 140.0;
/// The sheet stacks the dropdowns at the size a finger needs.
const SHEET_MODEL_WIDTH: f32 = 212.0;
const SHEET_THINKING_WIDTH: f32 = 152.0;
/// The width of the room that the top bar keeps for the two dropdowns side by side, so that the
/// second ends at the margin of the bar.
pub(super) const DROPDOWNS_WIDTH: f32 = BAR_MODEL_WIDTH + space::SM + BAR_THINKING_WIDTH;
/// The width of the room that the top bar keeps for the button that opens the sheet.
pub(super) const MODEL_BUTTON_WIDTH: f32 = 80.0;
/// The width of what the sheet holds, so that it fits the smallest window with its margins.
const SHEET_WIDTH: f32 = 260.0;
const BUSY_HINT: &str = "Model changes are available when the current operation finishes.";

/// Where the dropdowns are drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Look {
    /// Small and quiet, among the plain words of the top bar.
    Bar,
    /// At the size a finger needs, in the sheet.
    Sheet,
}

impl Look {
    /// The width of the Model and of the Thinking level dropdown.
    fn widths(self) -> (f32, f32) {
        match self {
            Look::Bar => (BAR_MODEL_WIDTH, BAR_THINKING_WIDTH),
            Look::Sheet => (SHEET_MODEL_WIDTH, SHEET_THINKING_WIDTH),
        }
    }

    fn dress<'a, S: AsRef<str>>(self, dropdown: Dropdown<'a, S>) -> Dropdown<'a, S> {
        match self {
            Look::Bar => dropdown.size(ControlSize::Small).quiet(),
            Look::Sheet => dropdown.size(ControlSize::Medium),
        }
    }
}

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
        // The dropdowns stand on the middle line of the bar, with the logo, the tabs and the
        // switch. A failed save puts a line under them, and then the two go up together.
        let top = match error {
            Some(_) => room.top() + 4.0,
            None => room.center().y - ControlSize::Small.height() / 2.0,
        };
        let bottom = if error.is_some() { 2.0 } else { 6.0 };
        let rect = egui::Rect::from_min_max(
            egui::pos2(room.left(), top),
            egui::pos2(room.right(), room.bottom() - bottom),
        );
        region(ui, "model_picker", rect, |ui| {
            if error.is_some() {
                ui.spacing_mut().item_spacing.y = 2.0;
            }
            let InnerResponse { inner, response } = ui.add_enabled_ui(is_idle, |ui| {
                ui.horizontal(|ui| self.dropdowns(ui, work, Look::Bar))
                    .inner
            });
            response.on_disabled_hover_text(BUSY_HINT);
            cx.intents.extend(inner);
            if let Some(error) = error {
                ui.label(TextRole::Small.rich(ui, "The model choice could not be saved."))
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
        let sheet = widgets::modal(ctx, egui::Id::new("model choices"), SHEET_WIDTH, |ui| {
            ui.label(TextRole::Heading.rich(ui, "Model choices"));
            ui.add_space(space::SM);
            let changed = ui
                .add_enabled_ui(is_idle, |ui| self.dropdowns(ui, work, Look::Sheet))
                .inner;
            cx.intents.extend(changed);
            // A touch screen has no hover, so the sheet shows in plain view what the bar shows
            // on hover. The hint of a failed save already says that the choice was not saved.
            if let Some(error) = error {
                ui.label(TextRole::Small.rich(ui, &error.hint));
            }
            busy_line(ui, is_idle);
            ui.add_space(space::SM);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(Button::primary("Done").size(ControlSize::Medium))
                    .clicked()
                {
                    ui.close();
                }
            });
        });
        if sheet.should_close() {
            self.is_open = false;
        }
    }

    /// The Model and the Thinking level dropdowns of `work`. A change is kept and returned as the
    /// intent that saves it.
    fn dropdowns(&mut self, ui: &mut egui::Ui, work: WorkKind, look: Look) -> Option<Intent> {
        let (model_width, thinking_width) = look.widths();
        let before = self.choices.clone();
        let choice = self.choices.of_mut(work);
        let models: Vec<_> = MODEL_CHOICES.iter().map(|(_, label)| *label).collect();
        let selected = MODEL_CHOICES.iter().position(|(id, _)| *id == choice.model);
        if let Some(index) = look
            .dress(Dropdown::new("Model", &models, model_width))
            .selected(selected)
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
        if let Some(index) = look
            .dress(Dropdown::new("Thinking level", &labels, thinking_width))
            .selected(selected)
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

/// The line that says why the choices wait, while they wait. While they do not, the line's room
/// stays empty, so that Done does not move under a finger when a running operation ends.
fn busy_line(ui: &mut egui::Ui, is_idle: bool) {
    let words = egui::WidgetText::from(TextRole::Small.rich(ui, BUSY_HINT));
    let galley = words.into_galley(ui, None, ui.available_width(), egui::TextStyle::Body);
    if is_idle {
        ui.allocate_space(galley.size());
    } else {
        ui.add(egui::Label::new(galley));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    /// A dropdown in the bar is only as wide as its longest choice, so a choice that grows longer
    /// is cut unless the width grows with it.
    #[test]
    fn the_longest_choice_fits_its_dropdown_in_the_bar() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut drawn = ctx.run_ui(egui::RawInput::default(), |ui| {
            let spacing = ui.spacing();
            // A quiet dropdown keeps the padding of a ghost button at each end.
            let chrome = spacing.icon_spacing + spacing.icon_width + 2.0 * space::SM;
            let needs = |text: &str| {
                TextRole::Label
                    .galley(ui, text, Color32::PLACEHOLDER)
                    .size()
                    .x
                    + chrome
            };
            for (_, model) in MODEL_CHOICES {
                let width = needs(model);
                assert!(width <= BAR_MODEL_WIDTH, "`{model}` needs {width}");
            }
            for level in THINKING_LEVELS {
                let width = needs(&format!("Thinking: {level}"));
                assert!(width <= BAR_THINKING_WIDTH, "`{level}` needs {width}");
            }
        });
        // Nothing paints the font texture, and egui asks that its change be dropped on purpose.
        drawn.textures_delta.clear();
    }
}
