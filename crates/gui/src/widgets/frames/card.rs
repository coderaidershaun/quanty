//! The entry that holds one result, one document or one step: a thin outline around its content.

use eframe::egui::{
    self, Align, Color32, Response, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, pos2, vec2,
};

use super::copyable::Copyable;
use crate::theme::{self, Icon, Kind, TextRole, Tone, radius, size, space, stroke};
use crate::widgets::controls::{Button, CopyButton};
use crate::widgets::look::{ControlSize, Look, focus_ring};

const MARGIN: f32 = space::MD;

pub struct Card<'a> {
    tag: Option<(Kind, &'a str)>,
    action: Option<(Icon, &'a str)>,
    action_size: ControlSize,
    copy: Option<&'a str>,
    is_selected: bool,
    clickable: Option<&'a str>,
    forced: Option<Look>,
}

pub struct CardResponse<R> {
    pub inner: R,
    pub response: Response,
    pub action_clicked: bool,
    /// True in the frame the copy button was clicked. The caller copies the text.
    pub copied: bool,
}

impl Default for Card<'_> {
    fn default() -> Self {
        Card {
            tag: None,
            action: None,
            action_size: ControlSize::Small,
            copy: None,
            is_selected: false,
            clickable: None,
            forced: None,
        }
    }
}

impl<'a> Card<'a> {
    pub fn new() -> Self {
        Card::default()
    }

    /// How much higher a card with a tag or an action is than what it holds: the header row, the
    /// gap under it, the margins and the border.
    pub fn height_around_content(ui: &egui::Ui) -> f32 {
        let spacing = ui.spacing();
        // The header is a row, and egui makes a row at least this high. The tag and an action
        // button of the default size are no higher than that.
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

    /// The size of the action button, `Small` unless a finger presses it.
    pub fn action_size(mut self, size: ControlSize) -> Self {
        self.action_size = size;
        self
    }

    /// A copy button at the top right, left of the action when there is one. `label` is its
    /// accessible name, and `None` gives no button.
    pub fn copy(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.copy = label.into();
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

    /// Draws the card as if the pointer or the keyboard were on it, unless `look` is `None`.
    /// Only a clickable card has a look to show.
    pub(in crate::widgets) fn preview(mut self, look: Option<Look>) -> Self {
        self.forced = look;
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
        let scope = ui.scope_builder(builder, |ui| self.framed(ui, add_contents));
        let (inner, action_clicked, copied) = scope.inner;
        crate::widgets::interactive_cursor(ui, &scope.response);
        if let Some(label) = self.clickable {
            scope.response.widget_info(|| {
                WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), self.is_selected, label)
            });
        }
        CardResponse {
            inner,
            response: scope.response,
            action_clicked,
            copied,
        }
    }

    /// The outline, the header and the content, in the `Ui` that senses a click on the whole
    /// card. Returns what the content returns, and whether the action button and the copy button
    /// were clicked.
    fn framed<R>(
        &self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> (R, bool, bool) {
        // The width of the outline never changes, so that selecting a card does not move what
        // is below it. The heavier outline of a selected card is drawn over it.
        let palette = theme::palette(ui.ctx());
        let mut frame = egui::Frame::NONE
            .stroke(Stroke::new(stroke::BORDER, palette.hairline))
            .corner_radius(radius::SM)
            .inner_margin(MARGIN)
            .begin(ui);
        let room = frame.content_ui.available_width();
        if room.is_finite() {
            frame.content_ui.set_min_width(room);
        }
        let mut clicks = HeaderClicks::default();
        let has_header = self.tag.is_some() || self.action.is_some();
        if has_header {
            clicks = self.header(&mut frame.content_ui);
        }
        let inner = match self.copy {
            Some(label) if !has_header => {
                let copyable = Copyable::new(label).show(&mut frame.content_ui, add_contents);
                clicks.copied = copyable.copied;
                copyable.inner
            }
            _ => add_contents(&mut frame.content_ui),
        };

        let look = match (self.clickable, self.forced) {
            (None, _) => Look::default(),
            (Some(_), Some(forced)) => forced,
            (Some(_), None) => Look::of(&ui.response()),
        };
        frame.frame.fill = if look.pressed {
            palette.well_deep
        } else if look.hovered {
            palette.well
        } else {
            Color32::TRANSPARENT
        };
        let outer = frame.content_ui.min_rect() + frame.frame.total_margin();
        frame.end(ui);
        if self.is_selected {
            let edge = Stroke::new(stroke::EDGE, palette.tone(Tone::Rubric).solid);
            ui.painter()
                .rect_stroke(outer, radius::SM, edge, StrokeKind::Inside);
        }
        if look.focused {
            focus_ring(ui.painter(), outer, radius::SM);
        }
        (inner, clicks.action, clicks.copied)
    }

    fn header(&self, ui: &mut egui::Ui) -> HeaderClicks {
        let mut clicks = HeaderClicks::default();
        ui.horizontal(|ui| {
            if let Some((kind, text)) = self.tag {
                paint_tag(ui, kind, text);
            }
            if self.action.is_none() && self.copy.is_none() {
                return;
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if let Some((icon, label)) = self.action {
                    let button = Button::icon_only(icon, label).size(self.action_size);
                    clicks.action = ui.add(button).clicked();
                }
                if let Some(label) = self.copy {
                    clicks.copied = ui.add(CopyButton::new(label)).clicked();
                }
            });
        });
        clicks
    }
}

#[derive(Default)]
struct HeaderClicks {
    action: bool,
    copied: bool,
}

/// The text of the tag is a label of its own, so a test or a screen reader finds it. It is
/// painted in capitals, and the name it answers to stays the text as it was given.
fn paint_tag(ui: &mut egui::Ui, kind: Kind, text: &str) {
    let ink = theme::palette(ui.ctx()).tone(kind.tone()).text;
    let galley = TextRole::Caps.galley(ui, &text.to_uppercase(), ink);
    let height = galley.size().y.max(size::GLYPH);
    let width = size::GLYPH + space::XS + galley.size().x;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    let glyph = pos2(rect.left() + size::GLYPH / 2.0, rect.center().y);
    kind.icon().paint(painter, glyph, size::GLYPH, ink);
    let left = rect.left() + size::GLYPH + space::XS;
    painter.galley(
        pos2(left, rect.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
}
