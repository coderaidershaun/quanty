//! The small tokens that sit in text and beside it: citation numbers, kind chips, badges, step
//! markers and the legend dot.

use eframe::egui::{
    self, Align2, Color32, Rect, Response, Stroke, StrokeKind, WidgetInfo, WidgetType, pos2,
    text::LayoutJob,
};

use crate::theme::{Icon, Kind, TextRole, Tone, color, radius, size, space, stroke};
use crate::widgets::look::{Look, focus_ring};

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

    /// Draws the chip as if the pointer or the keyboard were on it, unless `look` is `None`.
    pub(in crate::widgets) const fn preview(mut self, look: Option<Look>) -> Self {
        self.forced = look;
        self
    }

    /// The room the chip needs, to reserve in a flow of text.
    pub fn size(&self, ui: &egui::Ui) -> egui::Vec2 {
        let text = TextRole::Micro.galley(ui, &self.number.to_string(), color::TEXT);
        egui::vec2(text.size().x + 2.0 * space::XS, size::CITATION)
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
            let rubric = Tone::Rubric.swatch();
            let (fill, text) = if self.is_selected {
                (rubric.solid, rubric.on_solid)
            } else if look.hovered {
                (rubric.wash, rubric.text)
            } else {
                (Color32::TRANSPARENT, rubric.text)
            };
            let painter = ui.painter();
            painter.rect_filled(rect, radius::SM, fill);
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
        let (rect, response) = ui.allocate_exact_size(self.size(ui), egui::Sense::click());
        self.name_and_paint(ui, rect, response)
    }
}

pub struct Chip<'a> {
    label: &'a str,
    kind: Option<Kind>,
    is_selected: bool,
    is_stamp: bool,
    max_width: f32,
    forced: Option<Look>,
}

impl<'a> Chip<'a> {
    pub fn kind(kind: Kind, label: &'a str) -> Self {
        Chip {
            kind: Some(kind),
            ..Chip::plain(label)
        }
    }

    pub fn plain(label: &'a str) -> Self {
        Chip {
            label,
            kind: None,
            is_selected: false,
            is_stamp: false,
            max_width: f32::INFINITY,
            forced: None,
        }
    }

    /// A word that only labels, such as a tag. It looks like a plain chip at rest, but it cannot
    /// be clicked or focused, it does not change under the pointer, and a screen reader reads it
    /// as text.
    pub fn stamp(label: &'a str) -> Self {
        Chip {
            is_stamp: true,
            ..Chip::plain(label)
        }
    }

    /// A stamp ignores it, because a word that only labels is never chosen.
    pub fn selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected && !self.is_stamp;
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    /// Draws the chip as if the pointer or the keyboard were on it.
    pub(in crate::widgets) fn preview(mut self, look: Look) -> Self {
        self.forced = Some(look);
        self
    }

    /// Space before the text, space after it, and the room of the kind icon in between, if any.
    fn padding(&self) -> (f32, f32, f32) {
        match self.kind {
            Some(_) => (space::SM, space::SM, size::GLYPH + space::XS),
            None => (space::MD, space::MD, 0.0),
        }
    }

    fn ink(&self) -> Color32 {
        self.kind
            .map_or(color::TEXT, |kind| kind.tone().swatch().text)
    }

    fn paint(&self, ui: &egui::Ui, rect: Rect, look: Look) {
        let painter = ui.painter();
        let (fill, edge) = if self.is_selected {
            // Red marks the thread from a claim to its source, so only a kind chip takes it. A
            // plain chip is a choice, selected as a button is.
            let outline = match self.kind {
                Some(_) => Tone::Rubric.swatch().solid,
                None => color::RULE,
            };
            (color::WELL, Stroke::new(stroke::EDGE, outline))
        } else {
            let fill = if look.hovered {
                color::WELL
            } else {
                Color32::TRANSPARENT
            };
            (fill, Stroke::new(stroke::BORDER, color::HAIRLINE))
        };
        painter.rect(rect, radius::SM, fill, edge, StrokeKind::Inside);
        if look.focused {
            focus_ring(painter, rect, radius::SM);
        }
        if let Some(kind) = self.kind {
            let centre = pos2(rect.left() + space::SM + size::GLYPH / 2.0, rect.center().y);
            kind.icon().paint(painter, centre, size::GLYPH, self.ink());
        }
    }
}

impl egui::Widget for Chip<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (before, after, badge) = self.padding();
        let ink = self.ink();
        let mut job = LayoutJob::single_section(self.label.to_owned(), TextRole::Small.format(ink));
        job.wrap.max_rows = 1;
        job.wrap.overflow_character = Some('…');
        job.wrap.max_width = (self.max_width - before - badge - after).max(0.0);
        job.break_on_newline = false;
        let galley = ui.painter().layout_job(job);
        let side = egui::vec2(before + badge + galley.size().x + after, size::CHIP);
        let sense = if self.is_stamp {
            egui::Sense::hover()
        } else {
            egui::Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(side, sense);
        response.widget_info(|| {
            let is_enabled = ui.is_enabled();
            if self.is_stamp {
                WidgetInfo::labeled(WidgetType::Label, is_enabled, self.label)
            } else {
                WidgetInfo::selected(WidgetType::Button, is_enabled, self.is_selected, self.label)
            }
        });
        if ui.is_rect_visible(rect) {
            let look = if self.is_stamp {
                Look::default()
            } else {
                self.forced.unwrap_or_else(|| Look::of(&response))
            };
            self.paint(ui, rect, look);
            let top = rect.center().y - galley.size().y / 2.0;
            let left = rect.left() + before + badge;
            ui.painter().galley(pos2(left, top), galley.clone(), ink);
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
            tone: Tone::Ink,
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
        let ink = self.tone.swatch().text;
        let galley = TextRole::Caps.galley(ui, &self.text.to_uppercase(), ink);
        let icon_room = if self.icon.is_some() {
            size::GLYPH + space::XS
        } else {
            0.0
        };
        let side = egui::vec2(icon_room + galley.size().x, size::BADGE);
        let (rect, response) = ui.allocate_exact_size(side, egui::Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, self.text));
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let mut x = rect.left();
            if let Some(icon) = self.icon {
                icon.paint(
                    painter,
                    pos2(x + size::GLYPH / 2.0, rect.center().y),
                    size::GLYPH,
                    ink,
                );
                x += icon_room;
            }
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(pos2(x, top), galley, ink);
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
    state: StepState,
}

impl StepMarker {
    pub const fn new(number: usize) -> Self {
        StepMarker {
            number,
            state: StepState::Pending,
        }
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
            let painter = ui.painter();
            let ring = size::STEP / 2.0 - stroke::BORDER / 2.0;
            let text = match self.state {
                StepState::Pending => {
                    let outline = Stroke::new(stroke::BORDER, color::BORDER);
                    painter.circle_stroke(rect.center(), ring, outline);
                    color::TEXT_FAINT
                }
                StepState::Done => {
                    let outline = Stroke::new(stroke::BORDER, color::RULE);
                    painter.circle_stroke(rect.center(), ring, outline);
                    color::TEXT
                }
                StepState::Active => {
                    painter.circle_filled(rect.center(), size::STEP / 2.0, color::INK);
                    color::TEXT_ON_INK
                }
            };
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                self.number.to_string(),
                TextRole::Micro.font(),
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
