//! The raised card that holds one result, one document or one step.

use eframe::egui::{self, Response, WidgetInfo, WidgetType};

use super::Button;
use crate::theme::{Icon, Kind, TextRole, color, radius, space};

#[derive(Default)]
pub struct Card<'a> {
    tag: Option<(Kind, &'a str)>,
    action: Option<(Icon, &'a str)>,
    selected: bool,
    clickable: Option<&'a str>,
}

/// What a card reports. `action_clicked` is the icon button at its top right.
pub struct CardResponse<R> {
    pub inner: R,
    pub response: Response,
    pub action_clicked: bool,
}

impl<'a> Card<'a> {
    pub fn new() -> Self {
        Card::default()
    }

    /// A tag in the corner, naming what kind of thing the card holds.
    pub fn tag(mut self, kind: Kind, text: &'a str) -> Self {
        self.tag = Some((kind, text));
        self
    }

    /// An icon button at the top right. `label` is its accessible name.
    pub fn action(mut self, icon: Icon, label: &'a str) -> Self {
        self.action = Some((icon, label));
        self
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.selected = is_selected;
        self
    }

    /// The whole card answers to a click. `label` is its accessible name.
    pub fn clickable(mut self, label: &'a str) -> Self {
        self.clickable = Some(label);
        self
    }

    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> CardResponse<R> {
        let mut builder = egui::UiBuilder::new();
        if self.clickable.is_some() {
            builder = builder.sense(egui::Sense::click());
        }
        let mut action_clicked = false;
        let scope = ui.scope_builder(builder, |ui| {
            let stroke_color = if self.selected {
                crate::theme::Tone::Blue.swatch().solid
            } else {
                color::BORDER
            };
            egui::Frame::NONE
                .fill(color::RAISED)
                .stroke(egui::Stroke::new(1.0, stroke_color))
                .corner_radius(radius::LG)
                .inner_margin(space::MD)
                .show(ui, |ui| {
                    if self.tag.is_some() || self.action.is_some() {
                        ui.horizontal(|ui| {
                            if let Some((kind, text)) = self.tag {
                                ui.label(
                                    TextRole::Small.rich(text).color(kind.tone().swatch().text),
                                );
                            }
                            if let Some((icon, label)) = self.action {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        action_clicked =
                                            ui.add(Button::icon_only(icon, label)).clicked();
                                    },
                                );
                            }
                        });
                    }
                    add_contents(ui)
                })
                .inner
        });
        if let Some(label) = self.clickable {
            scope
                .response
                .widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
        }
        CardResponse {
            inner: scope.inner,
            response: scope.response,
            action_clicked,
        }
    }
}
