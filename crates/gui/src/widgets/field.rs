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

/// What a text box reports. `response` is that of the text itself, so its id is the one that
/// holds the focus. `submitted` is Enter while the box has the focus, and the box then gives the
/// focus up.
pub struct TextInputResponse {
    pub response: Response,
    pub submitted: bool,
    pub trailing_clicked: bool,
}

impl<'a> TextInput<'a> {
    /// `id_salt` must be different for each box in the same `Ui`, because the focus and the
    /// cursor are kept under it. `label` is the accessible name.
    pub fn new(id_salt: &'a str, label: &'a str, text: &'a mut String) -> Self {
        // SMELL: `id_salt` and `label` are both `&str`, so a call that swaps them still compiles.
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
        // SMELL: `_icon` is used, so its underscore is wrong. Rename it to `icon`.
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
        // A small box is lower than the smallest button, so the trailing button shrinks to fit.
        let trailing_side = inner_height.min(size::CONTROL_SM);
        // The frame makes room for its border when it begins, so the border gets its width
        // here, and only its colour changes once the focus is known.
        let mut frame = egui::Frame::NONE
            .fill(color::RAISED)
            .stroke(Stroke::new(stroke::BORDER, color::BORDER))
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
                        .map_or(0.0, |_| trailing_side + ui.spacing().item_spacing.x);
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
                        let button = Button::icon_only(icon, label).height(trailing_side);
                        trailing_clicked = ui.add(button).clicked();
                    }
                    response
                },
            )
            .inner;
        if response.has_focus() {
            frame.frame.stroke.color = color::FOCUS;
        }
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

/// `width`, or `fallback` when the room is endless, as it is inside a sideways scroll area.
fn finite_or(width: f32, fallback: f32) -> f32 {
    if width.is_finite() { width } else { fallback }
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
    /// `id_salt` must be different for each dropdown in the same `Ui`, because the open list is
    /// kept under it. `label` is the accessible name.
    pub fn new(id_salt: &'a str, label: &'a str, options: &'a [S]) -> Self {
        // SMELL: `id_salt` and `label` are both `&str`, so a call that swaps them still compiles.
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
        // SMELL: `_size` is used, so its underscore is wrong. Rename it to `size`.
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
        // SMELL: this lays out every choice on every frame. A dropdown with hundreds of
        // choices must be given a `width` instead.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    Previous,
    Next,
}

/// The centre text of a stepper is at least this wide, so "p. 9" and "p. 64" do not move it.
const STEPPER_TEXT_WIDTH: f32 = 48.0;
/// The size of the two buttons of a stepper, which is also how high the text between them is.
const STEPPER_SIZE: ControlSize = ControlSize::Small;

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

    /// The accessible name of the button that goes back, and whether it can be pressed.
    pub fn previous(mut self, label: &'a str, is_enabled: bool) -> Self {
        self.previous = (label, is_enabled);
        self
    }

    /// The accessible name of the button that goes on, and whether it can be pressed.
    pub fn next(mut self, label: &'a str, is_enabled: bool) -> Self {
        self.next = (label, is_enabled);
        self
    }

    /// The button the person just pressed.
    pub fn show(self, ui: &mut egui::Ui) -> Option<Step> {
        let mut step = None;
        egui::Frame::NONE
            .fill(color::RAISED)
            .stroke(Stroke::new(stroke::BORDER, color::BORDER))
            .corner_radius(radius::MD)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let side = STEPPER_SIZE.height();
                let text_width = TextRole::Label.galley(ui, self.text, color::TEXT).size().x;
                let middle = egui::vec2(text_width.max(STEPPER_TEXT_WIDTH), side);
                let whole = egui::vec2(side + middle.x + side, side);
                // A row takes the direction of the layout around it, so in a right-to-left
                // one it would put the next button first. The row stays, to put the stepper
                // where it always was, and the three parts get a row of their own inside it,
                // exactly as wide as they are, that always runs from left to right.
                let left_to_right = egui::Layout::left_to_right(Align::Center);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(whole, left_to_right, |ui| {
                        step = self.show_parts(ui, middle);
                    });
                });
            });
        step
    }

    /// Draws the previous button, the text and the next button, and says which button was
    /// pressed.
    fn show_parts(&self, ui: &mut egui::Ui, middle: egui::Vec2) -> Option<Step> {
        let mut step = None;
        let (label, is_enabled) = self.previous;
        let button = Button::icon_only(Icon::CARET_LEFT, label).size(STEPPER_SIZE);
        if ui.add_enabled(is_enabled, button).clicked() {
            step = Some(Step::Previous);
        }
        ui.allocate_ui_with_layout(
            middle,
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                // A label in a column wraps at the width it is given, and the room may be
                // rounded to a hair less than the text.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(TextRole::Label.rich(self.text))
            },
        );
        let (label, is_enabled) = self.next;
        let button = Button::icon_only(Icon::CARET_RIGHT, label).size(STEPPER_SIZE);
        if ui.add_enabled(is_enabled, button).clicked() {
            step = Some(Step::Next);
        }
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
        /// The room the box was given, and the room it took.
        room: Option<(egui::Rect, egui::Rect)>,
    }

    #[test]
    fn text_input_takes_typing_and_reports_enter() {
        let typed = Rc::new(RefCell::new(Typed::default()));
        let drawn = Rc::clone(&typed);
        let mut harness = testkit::panel([400.0, 80.0], Shared::default(), move |ui, _cx| {
            let mut typed = drawn.borrow_mut();
            let given = ui.available_rect_before_wrap();
            let shown = TextInput::new("ask", "Question", &mut typed.text)
                .placeholder("Ask the books")
                .icon(Icon::SEARCH)
                .trailing(Icon::CLOSE, "Clear question")
                .size(ControlSize::Small)
                .show(ui);
            typed.submits += usize::from(shown.submitted);
            typed.room = Some((given, ui.min_rect()));
        });
        harness.run();
        let (given, taken) = typed.borrow().room.expect("the box was drawn");
        let asked = egui::vec2(given.width(), ControlSize::Small.height());
        assert_eq!(
            taken,
            egui::Rect::from_min_size(given.min, asked),
            "the box starts where the layout put it, fills the width and is as high as its size"
        );

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
