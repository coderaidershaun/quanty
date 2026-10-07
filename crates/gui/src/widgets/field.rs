//! The widgets that take a value from the person: a text box, a dropdown, a slider and a pair
//! of step buttons.

use std::ops::RangeInclusive;

use eframe::egui::{self, Response};

use super::{Button, ControlSize};
use crate::theme::{Icon, TextRole, color};

pub struct TextInput<'a> {
    id_salt: &'a str,
    label: &'a str,
    text: &'a mut String,
    hint: Option<&'a str>,
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
            trailing: None,
            size: ControlSize::Medium,
            width: None,
        }
    }

    pub fn placeholder(mut self, hint: &'a str) -> Self {
        self.hint = Some(hint);
        self
    }

    pub fn icon(self, _icon: Icon) -> Self {
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
        let mut edit = egui::TextEdit::singleline(self.text)
            .id_salt(self.id_salt)
            .font(TextRole::Body.font())
            .min_size(egui::vec2(0.0, self.size.height()));
        if let Some(hint) = self.hint {
            edit = edit.hint_text(egui::RichText::new(hint).color(color::TEXT_MUTED));
        }
        if let Some(width) = self.width {
            edit = edit.desired_width(width);
        }
        let mut trailing_clicked = false;
        let response = ui
            .horizontal(|ui| {
                let response = ui.add(edit);
                if let Some((icon, label)) = self.trailing {
                    trailing_clicked = ui.add(Button::icon_only(icon, label)).clicked();
                }
                response
            })
            .inner;
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

pub struct Dropdown<'a, S> {
    id_salt: &'a str,
    label: &'a str,
    options: &'a [S],
    selected: Option<usize>,
    placeholder: &'a str,
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
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The index the person just chose.
    pub fn show(self, ui: &mut egui::Ui) -> Option<usize> {
        let shown = self
            .selected
            .and_then(|index| self.options.get(index))
            .map_or(self.placeholder, AsRef::as_ref);
        let mut chosen = None;
        let mut combo = egui::ComboBox::from_id_salt(self.id_salt).selected_text(shown);
        if let Some(width) = self.width {
            combo = combo.width(width).truncate();
        }
        // `truncate` cuts the text to the room it is given, and `width` is only a minimum, so
        // the room is capped here.
        let response = ui
            .scope(|ui| {
                if let Some(width) = self.width {
                    ui.set_max_width(width);
                }
                combo.show_ui(ui, |ui| {
                    for (index, option) in self.options.iter().enumerate() {
                        if ui
                            .selectable_label(self.selected == Some(index), option.as_ref())
                            .clicked()
                        {
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

    /// The value the person dragged to.
    pub fn show(mut self, ui: &mut egui::Ui) -> Option<f32> {
        let response = ui.add(egui::Slider::new(&mut self.value, self.range).show_value(false));
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
        ui.horizontal(|ui| {
            let (label, is_enabled) = self.previous;
            let button = Button::icon_only(Icon::CARET_LEFT, label);
            if ui.add_enabled(is_enabled, button).clicked() {
                step = Some(Step::Previous);
            }
            ui.label(TextRole::Label.rich(self.text));
            let (label, is_enabled) = self.next;
            let button = Button::icon_only(Icon::CARET_RIGHT, label);
            if ui.add_enabled(is_enabled, button).clicked() {
                step = Some(Step::Next);
            }
        });
        step
    }
}
