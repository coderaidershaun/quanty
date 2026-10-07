//! The widgets that take a value from the person: a text box, a dropdown, a slider and a pair
//! of step buttons.

use std::ops::RangeInclusive;

use eframe::egui::{self, Align, Margin, Response, Stroke};

use super::{Button, ControlSize};
use crate::theme::{Icon, TextRole, Tone, color, radius, size, space, stroke};

/// A one-line text box with a frame that shows the focus. It is the one widget that edits the
/// caller's own text, because egui needs a `&mut String`.
pub struct TextInput<'a> {
    id_salt: &'a str,
    label: &'a str,
    text: &'a mut String,
    hint: Option<&'a str>,
    leading: Option<Icon>,
    trailing: Option<(Icon, &'a str)>,
    size: ControlSize,
    width: Option<f32>,
}

/// What a text box reports. `submitted` is Enter while the box has the focus.
pub struct TextInputResponse {
    pub response: Response,
    pub submitted: bool,
    pub trailing_clicked: bool,
}

impl<'a> TextInput<'a> {
    pub fn new(id_salt: &'a str, label: &'a str, text: &'a mut String) -> Self {
        TextInput {
            id_salt,
            label,
            text,
            hint: None,
            leading: None,
            trailing: None,
            size: ControlSize::Medium,
            width: None,
        }
    }

    pub fn placeholder(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    /// An icon before the text.
    pub fn icon(self, _icon: Icon) -> Self {
        TextInput {
            leading: Some(_icon),
            ..self
        }
    }

    /// A button after the text, with an icon and an accessible label.
    pub fn trailing(mut self, icon: Icon, label: &'a str) -> Self {
        self.trailing = Some((icon, label));
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// The whole width of the box. Without it the box fills the width it is given.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Draws the box and reports Enter and the trailing button.
    pub fn show(self, ui: &mut egui::Ui) -> TextInputResponse {
        let outer_width = self
            .width
            .unwrap_or_else(|| finite_or(ui.available_width(), ui.spacing().text_edit_width));
        let inner_width = (outer_width - 2.0 * (space::MD + stroke::BORDER)).max(0.0);
        let inner_height = self.size.height() - 2.0 * stroke::BORDER;
        let mut frame = egui::Frame::NONE
            .fill(color::RAISED)
            .corner_radius(radius::MD)
            .inner_margin(Margin::from(egui::vec2(space::MD, 0.0)))
            .begin(ui);

        let mut trailing_clicked = false;
        let response = frame
            .content_ui
            .allocate_ui_with_layout(
                egui::vec2(inner_width, inner_height),
                egui::Layout::left_to_right(Align::Center),
                |ui| {
                    if let Some(icon) = self.leading {
                        let (_, rect) = ui.allocate_space(egui::Vec2::splat(size::ICON_MD));
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            icon.glyph(),
                            Icon::font(size::ICON_MD),
                            color::TEXT_SECONDARY,
                        );
                    }
                    let after = self
                        .trailing
                        .map_or(0.0, |_| size::CONTROL_SM + ui.spacing().item_spacing.x);
                    let mut edit = egui::TextEdit::singleline(self.text)
                        .id_salt(self.id_salt)
                        .font(TextRole::Body.font())
                        .frame(egui::Frame::NONE)
                        .margin(Margin::ZERO)
                        .vertical_align(Align::Center)
                        .desired_width((ui.available_width() - after).max(0.0))
                        .min_size(egui::vec2(0.0, inner_height));
                    if let Some(hint) = self.hint {
                        edit = edit.hint_text(hint);
                    }
                    let response = ui.add(edit);
                    if let Some((icon, label)) = self.trailing {
                        trailing_clicked = ui.add(Button::icon_only(icon, label)).clicked();
                    }
                    response
                },
            )
            .inner;
        let edge = if response.has_focus() {
            color::FOCUS
        } else {
            color::BORDER
        };
        frame.frame.stroke = Stroke::new(stroke::BORDER, edge);
        frame.end(ui);

        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(self.label));
        let submitted =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        TextInputResponse {
            response,
            submitted,
            trailing_clicked,
        }
    }
}

/// A closed box that opens a list of choices. It never keeps the choice: it reports the index
/// the person picked, and the caller passes the chosen one back in with `selected`.
pub struct Dropdown<'a, S> {
    id_salt: &'a str,
    label: &'a str,
    options: &'a [S],
    selected: Option<usize>,
    placeholder: &'a str,
    size: ControlSize,
    width: Option<f32>,
}

impl<'a, S: AsRef<str>> Dropdown<'a, S> {
    pub fn new(id_salt: &'a str, label: &'a str, options: &'a [S]) -> Self {
        Dropdown {
            id_salt,
            label,
            options,
            selected: None,
            placeholder: "",
            size: ControlSize::Small,
            width: None,
        }
    }

    /// `None` shows the placeholder.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = text;
        self
    }

    pub fn size(self, _size: ControlSize) -> Self {
        Dropdown {
            size: _size,
            ..self
        }
    }

    /// The whole width of the closed dropdown. A longer choice ends in "…". Without it the
    /// dropdown is as wide as its longest choice.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The index the person just chose.
    pub fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let chosen_text = self.selected.and_then(|index| self.options.get(index));
        let shown = match chosen_text {
            Some(option) => egui::RichText::new(option.as_ref()),
            None => egui::RichText::new(self.placeholder).color(color::TEXT_MUTED),
        };
        let mut chosen = None;
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().interact_size.y = self.size.height();
                let width = self.width.unwrap_or_else(|| self.widest(ui));
                // `truncate` cuts the text to the room it is given, and `width` is only a
                // minimum, so the room is capped here.
                ui.set_max_width(width);
                egui::ComboBox::from_id_salt(self.id_salt)
                    .selected_text(shown)
                    .width(width)
                    .truncate()
                    .icon(|ui, rect, _visuals, _is_open| {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            Icon::CARET_DOWN.glyph(),
                            Icon::font(size::ICON_SM),
                            color::TEXT_SECONDARY,
                        );
                    })
                    .show_ui(ui, |ui| {
                        for (index, option) in self.options.iter().enumerate() {
                            let is_selected = self.selected == Some(index);
                            if ui.selectable_label(is_selected, option.as_ref()).clicked() {
                                chosen = Some(index);
                            }
                        }
                    })
            })
            .inner;
        ui.ctx()
            .accesskit_node_builder(response.response.id, |node| node.set_label(self.label));
        chosen
    }

    /// The width that shows the longest choice, or the placeholder, in full.
    fn widest(&self, ui: &egui::Ui) -> f32 {
        let longest = self
            .options
            .iter()
            .map(AsRef::as_ref)
            .chain([self.placeholder])
            .map(|text| TextRole::Label.galley(ui, text, color::TEXT).size().x)
            .fold(0.0, f32::max);
        let spacing = ui.spacing();
        longest + spacing.icon_spacing + spacing.icon_width + 2.0 * spacing.button_padding.x
    }
}

/// A slider with no number next to it. It reports the value it was dragged to, and keeps none.
pub struct Slider<'a> {
    label: &'a str,
    value: f32,
    range: RangeInclusive<f32>,
}

impl<'a> Slider<'a> {
    pub fn new(label: &'a str, value: f32, range: RangeInclusive<f32>) -> Self {
        Slider {
            label,
            value,
            range,
        }
    }

    /// The value the person dragged to.
    pub fn show(mut self, ui: &mut egui::Ui) -> Option<f32> {
        let blue = Tone::Blue.swatch();
        let response = ui
            .scope(|ui| {
                // The stock slider paints its rail and its knob from the same field, so the
                // three states get the same fill here, and the knob gets a blue ring.
                let visuals = ui.visuals_mut();
                visuals.selection.bg_fill = blue.solid;
                for state in [
                    &mut visuals.widgets.inactive,
                    &mut visuals.widgets.hovered,
                    &mut visuals.widgets.active,
                ] {
                    state.bg_fill = color::BORDER;
                    state.fg_stroke = Stroke::new(stroke::FOCUS, blue.solid);
                }
                ui.add(egui::Slider::new(&mut self.value, self.range).show_value(false))
            })
            .inner;
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(self.label));
        response.changed().then_some(self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Previous,
    Next,
}

/// `width`, or `fallback` when the room is endless, as it is inside a sideways scroll area.
fn finite_or(width: f32, fallback: f32) -> f32 {
    if width.is_finite() { width } else { fallback }
}

/// The centre text of a stepper is at least this wide, so "p. 9" and "p. 64" do not move it.
const STEPPER_TEXT_WIDTH: f32 = 48.0;

/// A pair of previous and next buttons with a short text between them, such as a page number.
pub struct Stepper<'a> {
    text: &'a str,
    previous: (&'a str, bool),
    next: (&'a str, bool),
}

impl<'a> Stepper<'a> {
    /// `text` sits between the two buttons, such as "p. 64".
    pub fn new(text: &'a str) -> Self {
        Stepper {
            text,
            previous: ("Previous", true),
            next: ("Next", true),
        }
    }

    pub fn previous(mut self, label: &'a str, is_enabled: bool) -> Self {
        self.previous = (label, is_enabled);
        self
    }

    pub fn next(mut self, label: &'a str, is_enabled: bool) -> Self {
        self.next = (label, is_enabled);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Option<Step> {
        let mut step = None;
        egui::Frame::NONE
            .fill(color::RAISED)
            .stroke(Stroke::new(stroke::BORDER, color::BORDER))
            .corner_radius(radius::MD)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.horizontal(|ui| {
                    let (label, is_enabled) = self.previous;
                    let button = Button::icon_only(Icon::CARET_LEFT, label);
                    if ui.add_enabled(is_enabled, button).clicked() {
                        step = Some(Step::Previous);
                    }
                    let middle = egui::vec2(STEPPER_TEXT_WIDTH, size::CONTROL_SM);
                    ui.allocate_ui_with_layout(
                        middle,
                        egui::Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| ui.label(TextRole::Label.rich(self.text)),
                    );
                    let (label, is_enabled) = self.next;
                    let button = Button::icon_only(Icon::CARET_RIGHT, label);
                    if ui.add_enabled(is_enabled, button).clicked() {
                        step = Some(Step::Next);
                    }
                });
            });
        step
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::state::Shared;
    use crate::testkit;

    #[derive(Default)]
    struct Typed {
        text: String,
        submits: usize,
    }

    #[test]
    fn text_input_takes_typing_and_reports_enter() {
        let typed = Rc::new(RefCell::new(Typed::default()));
        let drawn = Rc::clone(&typed);
        let mut harness = testkit::panel([400.0, 80.0], Shared::default(), move |ui, _cx| {
            let mut typed = drawn.borrow_mut();
            let shown = TextInput::new("ask", "Question", &mut typed.text)
                .placeholder("Ask the books")
                .icon(Icon::SEARCH)
                .show(ui);
            typed.submits += usize::from(shown.submitted);
        });
        harness.run();

        let input = harness.get_by_label("Question");
        input.click();
        harness.run();
        harness
            .get_by_label("Question")
            .type_text("derive Black-Scholes");
        harness.run();
        assert_eq!(typed.borrow().text, "derive Black-Scholes");
        assert_eq!(typed.borrow().submits, 0, "typing alone is not a submit");

        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(typed.borrow().submits, 1);
        assert_eq!(typed.borrow().text, "derive Black-Scholes");
    }
}
