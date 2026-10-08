//! The small tokens that sit in text and beside it: citation numbers, kind chips, badges, step
//! markers and the legend dot.

use eframe::egui::{
    self, Align2, Rect, Response, Stroke, StrokeKind, WidgetInfo, WidgetType, pos2, text::LayoutJob,
};

use super::look::{Look, focus_ring};
use crate::theme::{Icon, Kind, TextRole, Tone, color, glow, radius, size, space, stroke};

const CHIP_GLYPH: f32 = 10.0;
const CHIP_BADGE: f32 = 16.0;

pub struct CitationChip {
    number: usize,
    is_selected: bool,
    forced: Option<Look>,
}

impl CitationChip {
    pub const fn new(number: usize) -> Self {
        CitationChip {
            number,
            is_selected: false,
            forced: None,
        }
    }

    pub const fn selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }

    /// Draws the chip as if the pointer or the keyboard were on it.
    pub(super) const fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
    }

    /// The room the chip needs, to reserve in a flow of text.
    pub fn size(self, ui: &egui::Ui) -> egui::Vec2 {
        self.measure(ui)
    }

    fn measure(&self, ui: &egui::Ui) -> egui::Vec2 {
        let text = TextRole::Micro.galley(ui, &self.number.to_string(), color::TEXT);
        egui::vec2(
            size::CITATION.max(text.size().x + 2.0 * space::XS),
            size::CITATION,
        )
    }

    /// Reacts and paints at `rect`, for a chip inside text that is laid out by hand. The layout
    /// cursor does not move. Call it after the text is painted, so the chip gets the click.
    pub fn show_at(self, ui: &egui::Ui, rect: egui::Rect, id: egui::Id) -> Response {
        let response = ui.interact(rect, id, egui::Sense::click());
        self.name_and_paint(ui, rect, response)
    }

    fn name_and_paint(&self, ui: &egui::Ui, rect: Rect, response: Response) -> Response {
        response.widget_info(|| {
            let label = format!("Citation {}", self.number);
            WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), self.is_selected, label)
        });
        if ui.is_rect_visible(rect) {
            let look = self.forced.unwrap_or_else(|| Look::of(&response));
            let blue = Tone::Blue.swatch();
            let (fill, text) = if self.is_selected {
                (blue.solid, blue.on_solid)
            } else if look.hovered {
                (blue.edge, color::TEXT)
            } else {
                (blue.wash, color::TEXT)
            };
            let painter = ui.painter();
            painter.rect(
                rect,
                radius::SM,
                fill,
                Stroke::new(stroke::BORDER, blue.edge),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.number.to_string(),
                TextRole::Micro.font(),
                text,
            );
            if look.focused {
                focus_ring(painter, rect, radius::SM);
            }
        }
        response
    }
}

impl egui::Widget for CitationChip {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(self.measure(ui), egui::Sense::click());
        self.name_and_paint(ui, rect, response)
    }
}

pub struct Chip<'a> {
    label: &'a str,
    kind: Option<Kind>,
    is_selected: bool,
    max_width: f32,
    forced: Option<Look>,
}

impl<'a> Chip<'a> {
    pub fn kind(kind: Kind, label: &'a str) -> Self {
        Chip {
            label,
            kind: Some(kind),
            is_selected: false,
            max_width: f32::INFINITY,
            forced: None,
        }
    }

    pub fn plain(label: &'a str) -> Self {
        Chip {
            label,
            kind: None,
            is_selected: false,
            max_width: f32::INFINITY,
            forced: None,
        }
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    /// Draws the chip as if the pointer or the keyboard were on it.
    pub(super) fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
    }

    /// Space before the text, space after it, and the round badge in between, if any.
    fn padding(&self) -> (f32, f32, f32) {
        match self.kind {
            Some(_) => (space::XS, space::SM, CHIP_BADGE + space::SM),
            None => (space::MD, space::MD, 0.0),
        }
    }

    fn paint(&self, ui: &egui::Ui, rect: Rect, look: Look) {
        let painter = ui.painter();
        let (fill, edge, corner) = match self.kind {
            Some(kind) => {
                let swatch = kind.tone().swatch();
                let edge = if self.is_selected {
                    Stroke::new(stroke::EDGE, swatch.solid)
                } else if look.hovered {
                    Stroke::new(stroke::BORDER, swatch.solid)
                } else {
                    Stroke::new(stroke::BORDER, swatch.edge)
                };
                if self.is_selected {
                    painter.add(glow(kind.tone()).as_shape(rect, radius::MD));
                }
                (swatch.wash, edge, radius::MD)
            }
            None if self.is_selected => {
                let blue = Tone::Blue.swatch();
                (blue.wash, Stroke::new(stroke::EDGE, blue.solid), radius::XL)
            }
            None => {
                let fill = if look.hovered {
                    color::RAISED_HOVER
                } else {
                    color::RAISED
                };
                (fill, Stroke::new(stroke::BORDER, color::BORDER), radius::XL)
            }
        };
        painter.rect(rect, corner, fill, edge, StrokeKind::Inside);
        if look.focused {
            focus_ring(painter, rect, corner);
        }
        if let Some(kind) = self.kind {
            let swatch = kind.tone().swatch();
            let centre = pos2(rect.left() + space::XS + CHIP_BADGE / 2.0, rect.center().y);
            painter.circle_filled(centre, CHIP_BADGE / 2.0, swatch.solid);
            painter.text(
                centre,
                Align2::CENTER_CENTER,
                kind.icon().glyph(),
                Icon::font(CHIP_GLYPH),
                swatch.on_solid,
            );
        }
    }
}

impl egui::Widget for Chip<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (before, after, badge) = self.padding();
        let mut job =
            LayoutJob::single_section(self.label.to_owned(), TextRole::Small.format(color::TEXT));
        job.wrap.max_rows = 1;
        job.wrap.overflow_character = Some('…');
        job.wrap.max_width = (self.max_width - before - badge - after).max(0.0);
        job.break_on_newline = false;
        let galley = ui.painter().layout_job(job);
        let side = egui::vec2(before + badge + galley.size().x + after, size::CHIP);
        let (rect, response) = ui.allocate_exact_size(side, egui::Sense::click());
        response.widget_info(|| {
            let is_enabled = ui.is_enabled();
            WidgetInfo::selected(WidgetType::Button, is_enabled, self.is_selected, self.label)
        });
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, self.forced.unwrap_or_else(|| Look::of(&response)));
            let top = rect.center().y - galley.size().y / 2.0;
            let left = rect.left() + before + badge;
            ui.painter()
                .galley(pos2(left, top), galley.clone(), color::TEXT);
        }
        if galley.elided {
            response.on_hover_text(self.label)
        } else {
            response
        }
    }
}

pub struct Badge<'a> {
    text: &'a str,
    tone: Tone,
    icon: Option<Icon>,
}

impl<'a> Badge<'a> {
    pub const fn new(text: &'a str) -> Self {
        Badge {
            text,
            tone: Tone::Neutral,
            icon: None,
        }
    }

    pub const fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub const fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
}

impl egui::Widget for Badge<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let swatch = self.tone.swatch();
        let galley = TextRole::Small.galley(ui, self.text, swatch.text);
        let icon_room = if self.icon.is_some() {
            size::ICON_SM + space::XS
        } else {
            0.0
        };
        let side = egui::vec2(icon_room + galley.size().x + 2.0 * space::SM, size::BADGE);
        let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, self.text));
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect(
                rect,
                radius::MD,
                swatch.wash,
                Stroke::new(stroke::BORDER, swatch.edge),
                StrokeKind::Inside,
            );
            let mut x = rect.left() + space::SM;
            if let Some(icon) = self.icon {
                painter.text(
                    pos2(x + size::ICON_SM / 2.0, rect.center().y),
                    Align2::CENTER_CENTER,
                    icon.glyph(),
                    Icon::font(size::ICON_SM),
                    swatch.text,
                );
                x += icon_room;
            }
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(pos2(x, top), galley, swatch.text);
        }
        response
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepState {
    Done,
    Active,
    Pending,
}

pub struct StepMarker {
    number: usize,
    tone: Tone,
    state: StepState,
}

impl StepMarker {
    pub const fn new(number: usize) -> Self {
        StepMarker {
            number,
            tone: Tone::Blue,
            state: StepState::Pending,
        }
    }

    pub const fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub const fn state(mut self, state: StepState) -> Self {
        self.state = state;
        self
    }
}

impl egui::Widget for StepMarker {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let side = egui::Vec2::splat(size::STEP);
        let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
        response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::Label, true, format!("Step {}", self.number))
        });
        if ui.is_rect_visible(rect) {
            let swatch = self.tone.swatch();
            let painter = ui.painter();
            let text = match self.state {
                StepState::Pending => {
                    painter.circle_stroke(
                        rect.center(),
                        size::STEP / 2.0 - stroke::BORDER / 2.0,
                        Stroke::new(stroke::BORDER, color::BORDER),
                    );
                    color::TEXT_MUTED
                }
                StepState::Done | StepState::Active => {
                    if self.state == StepState::Active {
                        painter.add(glow(self.tone).as_shape(rect, size::STEP / 2.0));
                    }
                    painter.circle_filled(rect.center(), size::STEP / 2.0, swatch.solid);
                    swatch.on_solid
                }
            };
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.number.to_string(),
                TextRole::Label.font(),
                text,
            );
        }
        response
    }
}

/// A filled circle in a tone. It has no label and no click: put a text beside it.
pub fn dot(ui: &mut egui::Ui, tone: Tone) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(size::DOT), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter()
            .circle_filled(rect.center(), size::DOT / 2.0, tone.swatch().solid);
    }
    response
}
