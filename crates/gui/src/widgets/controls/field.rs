//! The widgets that take a value from the person: a text box, a dropdown, a slider and a pair
//! of step buttons.

use std::ops::RangeInclusive;

use eframe::egui::{self, Align, Color32, CornerRadius, Margin, Response, Stroke};

use super::Button;
use crate::theme::{self, Icon, TextRole, radius, size, space, stroke};
use crate::widgets::ControlSize;
use crate::widgets::look::focus_ring;

/// It is the one widget that edits the caller's own text, because egui needs a `&mut String`.
pub struct TextInput<'a> {
    label: &'a str,
    id_salt: &'a str,
    text: &'a mut String,
    hint: Option<&'a str>,
    leading: Option<Icon>,
    trailing: Option<(Icon, &'a str)>,
    size: ControlSize,
    width: Option<f32>,
}

/// `response` is that of the text itself, so its id is the one that holds the focus. `submitted`
/// is Enter while the box has the focus, and the box then gives the focus up.
pub struct TextInputResponse {
    pub response: Response,
    pub submitted: bool,
    pub trailing_clicked: bool,
}

impl<'a> TextInput<'a> {
    /// `label` is the accessible name. The focus and the cursor are kept under it, so a box
    /// that shares its label with another box in the same `Ui`, or whose label changes, needs
    /// an `id_salt`.
    pub fn new(label: &'a str, text: &'a mut String) -> Self {
        TextInput {
            label,
            id_salt: label,
            text,
            hint: None,
            leading: None,
            trailing: None,
            size: ControlSize::Medium,
            width: None,
        }
    }

    /// The name the focus and the cursor are kept under, in place of the label.
    pub fn id_salt(mut self, id_salt: &'a str) -> Self {
        self.id_salt = id_salt;
        self
    }

    pub fn placeholder(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.leading = Some(icon);
        self
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

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> TextInputResponse {
        let outer_width = self
            .width
            .unwrap_or_else(|| finite_or(ui.available_width(), ui.spacing().text_edit_width));
        let inner_width = (outer_width - 2.0 * (space::MD + stroke::BORDER)).max(0.0);
        let inner_height = self.size.height() - 2.0 * stroke::BORDER;
        let label = self.label;
        let palette = theme::palette(ui.ctx());
        // The frame makes room for its border when it begins, so the border gets its width
        // here, and only its colour changes once the focus is known.
        let mut frame = egui::Frame::NONE
            .fill(palette.sheet)
            .stroke(Stroke::new(stroke::BORDER, palette.border))
            .corner_radius(radius::SM)
            .inner_margin(Margin::from(egui::vec2(space::MD, 0.0)))
            .begin(ui);

        let (response, trailing) = frame
            .content_ui
            .allocate_ui_with_layout(
                egui::vec2(inner_width, inner_height),
                egui::Layout::left_to_right(Align::Center),
                |ui| self.show_row(ui, inner_height),
            )
            .inner;
        let has_focus = response.has_focus();
        if has_focus {
            frame.frame.stroke.color = palette.rule;
        }
        let outer = frame.end(ui).rect;
        if has_focus {
            focus_ring(ui.painter(), outer, radius::SM);
        }

        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(label));
        let submitted =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        TextInputResponse {
            response,
            submitted,
            trailing_clicked: trailing.is_some_and(|button| button.clicked()),
        }
    }

    /// The row inside the frame: the leading icon, the text, and the trailing button if there is
    /// one. Returns the response of the text and that of the button.
    fn show_row(self, ui: &mut egui::Ui, height: f32) -> (Response, Option<Response>) {
        // A small box is lower than the smallest button, so the trailing button shrinks to fit. A
        // large box has room for a button that a finger can press.
        let tallest_button = match self.size {
            ControlSize::Large => size::CONTROL_MD,
            ControlSize::Small | ControlSize::Medium => size::CONTROL_SM,
        };
        let trailing_side = height.min(tallest_button);
        if let Some(icon) = self.leading {
            let (_, rect) = ui.allocate_space(egui::Vec2::splat(size::ICON_MD));
            icon.paint(
                ui.painter(),
                rect.center(),
                size::ICON_MD,
                theme::palette(ui.ctx()).text_secondary,
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
            .min_size(egui::vec2(0.0, height));
        if let Some(hint) = self.hint {
            edit = edit.hint_text(hint);
        }
        let response = ui.add(edit);
        let trailing = self
            .trailing
            .map(|(icon, label)| ui.add(Button::icon_only(icon, label).height(trailing_side)));
        (response, trailing)
    }
}

/// `width`, or `fallback` when the room is endless, as it is inside a sideways scroll area.
fn finite_or(width: f32, fallback: f32) -> f32 {
    if width.is_finite() { width } else { fallback }
}

// SMELL: this file is over 400 lines. The dropdown, the slider and the stepper from here down are
// the part to move out first, so that this file keeps only the text box.

/// A closed box that opens a list of choices. It never keeps the choice: it reports the index
/// the person picked, and the caller passes the chosen one back in with `selected`.
pub struct Dropdown<'a, S> {
    label: &'a str,
    id_salt: &'a str,
    options: &'a [S],
    selected: Option<usize>,
    placeholder: &'a str,
    size: ControlSize,
    width: f32,
    is_quiet: bool,
}

impl<'a, S: AsRef<str>> Dropdown<'a, S> {
    /// `label` is the accessible name. The open list is kept under it, so a dropdown that
    /// shares its label with another one in the same `Ui`, or whose label changes, needs an
    /// `id_salt`. `width` is fixed, so a long choice is cut and never moves what stands beside
    /// the box.
    pub fn new(label: &'a str, options: &'a [S], width: f32) -> Self {
        Dropdown {
            label,
            id_salt: label,
            options,
            selected: None,
            placeholder: "",
            size: ControlSize::Small,
            width,
            is_quiet: false,
        }
    }

    /// The name the open list is kept under, in place of the label.
    pub fn id_salt(mut self, id_salt: &'a str) -> Self {
        self.id_salt = id_salt;
        self
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = text;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// For a bar of plain words, such as the top bar: at rest the box has no fill and only a
    /// hairline edge, and it keeps the narrow padding of a ghost button. The pointer and an open
    /// list draw it as any other dropdown.
    pub fn quiet(mut self) -> Self {
        self.is_quiet = true;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let chosen_text = self.selected.and_then(|index| self.options.get(index));
        let muted = theme::palette(ui.ctx()).text_muted;
        let shown = match chosen_text {
            Some(option) => egui::RichText::new(option.as_ref()),
            None => egui::RichText::new(self.placeholder).color(muted),
        };
        let mut chosen = None;
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().interact_size.y = self.size.height();
                if self.is_quiet {
                    // The open list is drawn in a layer of its own, with the style of the app.
                    ui.spacing_mut().button_padding.x = QUIET_PADDING;
                    let hairline = theme::palette(ui.ctx()).hairline;
                    let rest = &mut ui.visuals_mut().widgets.inactive;
                    rest.weak_bg_fill = Color32::TRANSPARENT;
                    rest.bg_stroke = Stroke::new(stroke::BORDER, hairline);
                }
                // `truncate` cuts the text to the room it is given, and `width` is only a
                // minimum, so the room is capped here.
                ui.set_max_width(self.width);
                egui::ComboBox::from_id_salt(self.id_salt)
                    .selected_text(shown)
                    .width(self.width)
                    .truncate()
                    .icon(paint_caret)
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
        crate::widgets::interactive_cursor(ui, &response.response);
        ui.ctx()
            .accesskit_node_builder(response.response.id, |node| node.set_label(self.label));
        chosen
    }
}

/// The padding of a ghost button, at each end of a quiet dropdown.
const QUIET_PADDING: f32 = space::SM;

/// The caret at the right end of a closed list, for `ComboBox::icon`.
pub fn paint_caret(ui: &egui::Ui, rect: egui::Rect, _: &egui::style::WidgetVisuals, _: bool) {
    Icon::CARET_DOWN.paint(
        ui.painter(),
        rect.center(),
        size::ICON_SM,
        theme::palette(ui.ctx()).text_secondary,
    );
}

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

    pub fn show(mut self, ui: &mut egui::Ui) -> Option<f32> {
        let response = ui
            .scope(|ui| {
                // The stock slider paints its rail and the knob at rest from the same field, so
                // the knob is a rail-coloured disc with an ink ring, and it fills with ink while
                // the pointer is on it. The rail has square ends, like a progress bar.
                let palette = theme::palette(ui.ctx());
                let visuals = ui.visuals_mut();
                visuals.selection.bg_fill = palette.ink;
                visuals.widgets.inactive.bg_fill = palette.hairline;
                visuals.widgets.inactive.corner_radius = CornerRadius::ZERO;
                visuals.widgets.hovered.bg_fill = palette.ink_hover;
                visuals.widgets.active.bg_fill = palette.ink_pressed;
                for state in [
                    &mut visuals.widgets.inactive,
                    &mut visuals.widgets.hovered,
                    &mut visuals.widgets.active,
                ] {
                    state.fg_stroke = Stroke::new(stroke::FOCUS, palette.ink);
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
const STEPPER_SIZE: ControlSize = ControlSize::Small;

pub struct Stepper<'a> {
    text: &'a str,
    previous: (&'a str, bool),
    next: (&'a str, bool),
    size: ControlSize,
}

impl<'a> Stepper<'a> {
    /// `text` sits between the two buttons, such as "p. 64".
    pub fn new(text: &'a str) -> Self {
        Stepper {
            text,
            previous: ("Previous", true),
            next: ("Next", true),
            size: STEPPER_SIZE,
        }
    }

    /// The height of the stepper and the width of each button. A stepper that a finger presses
    /// is `ControlSize::Medium`.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// `label` is the accessible name of the button that goes back.
    pub fn previous(mut self, label: &'a str, is_enabled: bool) -> Self {
        self.previous = (label, is_enabled);
        self
    }

    /// `label` is the accessible name of the button that goes on.
    pub fn next(mut self, label: &'a str, is_enabled: bool) -> Self {
        self.next = (label, is_enabled);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> Option<Step> {
        let mut step = None;
        let palette = theme::palette(ui.ctx());
        egui::Frame::NONE
            .fill(palette.sheet)
            .stroke(Stroke::new(stroke::BORDER, palette.border))
            .corner_radius(radius::SM)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let side = self.size.height();
                let text_width = TextRole::Body
                    .galley(ui, self.text, Color32::PLACEHOLDER)
                    .size()
                    .x;
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

    fn show_parts(&self, ui: &mut egui::Ui, middle: egui::Vec2) -> Option<Step> {
        let mut step = None;
        let (label, is_enabled) = self.previous;
        let button = Button::icon_only(Icon::CARET_LEFT, label).size(self.size);
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
                ui.label(TextRole::Body.rich(ui, self.text))
            },
        );
        let (label, is_enabled) = self.next;
        let button = Button::icon_only(Icon::CARET_RIGHT, label).size(self.size);
        if ui.add_enabled(is_enabled, button).clicked() {
            step = Some(Step::Next);
        }
        step
    }
}
