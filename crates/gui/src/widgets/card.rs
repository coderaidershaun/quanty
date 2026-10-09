//! The raised card that holds one result, one document or one step.

use eframe::egui::{
    self, Align, Align2, Response, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, pos2, vec2,
};

use super::Button;
use super::look::{Look, focus_ring};
use crate::theme::{Icon, Kind, TextRole, Tone, color, radius, size, space, stroke};

const TAG_GLYPH: f32 = 10.0;
const TAG_BOX: f32 = 16.0;
const MARGIN: f32 = space::MD;

#[derive(Default)]
pub struct Card<'a> {
    tag: Option<(Kind, &'a str)>,
    action: Option<(Icon, &'a str)>,
    is_selected: bool,
    clickable: Option<&'a str>,
    forced: Option<Look>,
}

pub struct CardResponse<R> {
    pub inner: R,
    pub response: Response,
    pub action_clicked: bool,
}

impl<'a> Card<'a> {
    pub fn new() -> Self {
        Card::default()
    }

    /// How much higher a card with a tag or an action is than what it holds: the header row, the
    /// gap under it, the margins and the border.
    pub fn height_around_content(ui: &egui::Ui) -> f32 {
        let spacing = ui.spacing();
        // The header is a row, and egui makes a row at least this high. The tag and the action
        // button are no higher than that.
        let header = spacing.interact_size.y;
        header + spacing.item_spacing.y + 2.0 * (MARGIN + stroke::BORDER)
    }

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
        self.is_selected = is_selected;
        self
    }

    /// `label` is the accessible name of the card.
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
            let edge = if self.is_selected {
                Tone::Blue.swatch().solid
            } else {
                color::BORDER
            };
            let mut frame = egui::Frame::NONE
                .stroke(Stroke::new(stroke::BORDER, edge))
                .corner_radius(radius::LG)
                .inner_margin(MARGIN)
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
            scope.response.widget_info(|| {
                WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), self.is_selected, label)
            });
        }
        CardResponse {
            inner: scope.inner,
            response: scope.response,
            action_clicked,
        }
    }

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

/// The text of the tag is a label of its own, so a test or a screen reader finds it.
fn paint_tag(ui: &mut egui::Ui, kind: Kind, text: &str) {
    let swatch = kind.tone().swatch();
    let galley = TextRole::Small.galley(ui, text, swatch.text);
    let inset = (size::CHIP - TAG_BOX) / 2.0;
    let width = inset + TAG_BOX + space::SM + galley.size().x + space::SM;
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::CHIP), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
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
