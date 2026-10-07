//! The raised card that holds one result, one document or one step.

use eframe::egui::{
    self, Align, Align2, Response, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, pos2, vec2,
};

use super::Button;
use super::look::{Look, focus_ring};
use crate::theme::{Icon, Kind, TextRole, Tone, color, radius, size, space, stroke};

/// The icon inside the small box of a card tag.
const TAG_GLYPH: f32 = 10.0;
/// The small box of a card tag.
const TAG_BOX: f32 = 16.0;

#[derive(Default)]
pub struct Card<'a> {
    tag: Option<(Kind, &'a str)>,
    action: Option<(Icon, &'a str)>,
    selected: bool,
    clickable: Option<&'a str>,
    forced: Option<Look>,
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

    /// Draws the card as if the pointer or the keyboard were on it. Only a clickable card has a
    /// look to show.
    pub(super) fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
    }

    /// Draws the card around whatever `add_contents` draws.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> CardResponse<R> {
        let mut builder = egui::UiBuilder::new();
        if self.clickable.is_some() {
            builder = builder.sense(Sense::click());
        }
        let mut action_clicked = false;
        let scope = ui.scope_builder(builder, |ui| {
            let edge = if self.selected {
                Tone::Blue.swatch().solid
            } else {
                color::BORDER
            };
            let mut frame = egui::Frame::NONE
                .stroke(Stroke::new(stroke::BORDER, edge))
                .corner_radius(radius::LG)
                .inner_margin(space::MD)
                .begin(ui);
            let room = frame.content_ui.available_width();
            if room.is_finite() {
                frame.content_ui.set_min_width(room);
            }
            if self.tag.is_some() || self.action.is_some() {
                action_clicked = self.header(&mut frame.content_ui);
            }
            let inner = add_contents(&mut frame.content_ui);

            let look = match (self.clickable, self.forced) {
                (None, _) => Look::default(),
                (Some(_), Some(forced)) => forced,
                (Some(_), None) => Look::of(&ui.response()),
            };
            frame.frame.fill = if look.hovered || look.pressed {
                color::RAISED_HOVER
            } else {
                color::RAISED
            };
            let outer = frame.content_ui.min_rect() + frame.frame.total_margin();
            frame.end(ui);
            if look.focused {
                focus_ring(ui.painter(), outer, radius::LG);
            }
            inner
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

    /// The row above the contents: the tag at the left, the action at the right. Says whether
    /// the action was clicked.
    fn header(&self, ui: &mut egui::Ui) -> bool {
        let mut action_clicked = false;
        ui.horizontal(|ui| {
            if let Some((kind, text)) = self.tag {
                paint_tag(ui, kind, text);
            }
            if let Some((icon, label)) = self.action {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    action_clicked = ui.add(Button::icon_only(icon, label)).clicked();
                });
            }
        });
        action_clicked
    }
}

/// The corner tag: a small coloured box with the kind's icon, then its name.
fn paint_tag(ui: &mut egui::Ui, kind: Kind, text: &str) {
    let swatch = kind.tone().swatch();
    let galley = TextRole::Small.galley(ui, text, swatch.text);
    let inset = (size::CHIP - TAG_BOX) / 2.0;
    let width = inset + TAG_BOX + space::SM + galley.size().x + space::SM;
    let (rect, _) = ui.allocate_exact_size(vec2(width, size::CHIP), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect(
        rect,
        radius::MD,
        swatch.wash,
        Stroke::new(stroke::BORDER, swatch.edge),
        StrokeKind::Inside,
    );
    let tag_box = egui::Rect::from_min_size(
        pos2(rect.left() + inset, rect.center().y - TAG_BOX / 2.0),
        egui::Vec2::splat(TAG_BOX),
    );
    painter.rect_filled(tag_box, radius::SM, swatch.solid);
    painter.text(
        tag_box.center(),
        Align2::CENTER_CENTER,
        kind.icon().glyph(),
        Icon::font(TAG_GLYPH),
        swatch.on_solid,
    );
    let left = tag_box.right() + space::SM;
    painter.galley(
        pos2(left, rect.center().y - galley.size().y / 2.0),
        galley,
        swatch.text,
    );
}
