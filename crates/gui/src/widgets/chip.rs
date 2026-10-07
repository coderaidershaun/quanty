//! The small tokens that sit in text and beside it: citation numbers, kind chips, badges, step
//! markers and the legend dot.

use eframe::egui::{self, Response, WidgetInfo, WidgetType};

use crate::theme::{Icon, Kind, TextRole, Tone, color, radius, size, space};

/// The number of a source, as a clickable chip. Its accessible name is "Citation {n}".
pub struct CitationChip {
    number: usize,
    selected: bool,
}

impl CitationChip {
    pub const fn new(number: usize) -> Self {
        CitationChip {
            number,
            selected: false,
        }
    }

    pub const fn selected(mut self, is_selected: bool) -> Self {
        self.selected = is_selected;
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

    /// Reacts and paints at `rect`. The layout cursor does not move.
    pub fn show_at(self, ui: &egui::Ui, rect: egui::Rect, id: egui::Id) -> Response {
        let response = ui.interact(rect, id, egui::Sense::click());
        self.finish(ui, rect, response)
    }

    fn finish(&self, ui: &egui::Ui, rect: egui::Rect, response: Response) -> Response {
        let label = format!("Citation {}", self.number);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
        if ui.is_rect_visible(rect) {
            let swatch = Tone::Blue.swatch();
            let (fill, text) = if self.selected {
                (swatch.solid, swatch.on_solid)
            } else if response.hovered() {
                (swatch.edge, color::TEXT)
            } else {
                (swatch.wash, color::TEXT)
            };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, radius::SM, fill);
            painter.rect_stroke(
                rect,
                radius::SM,
                egui::Stroke::new(1.0, swatch.edge),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.number.to_string(),
                TextRole::Micro.font(),
                text,
            );
        }
        response
    }
}

impl egui::Widget for CitationChip {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(self.measure(ui), egui::Sense::click());
        self.finish(ui, rect, response)
    }
}

/// A clickable label, with a colour and an icon when it stands for a kind.
pub struct Chip<'a> {
    label: &'a str,
    kind: Option<Kind>,
    selected: bool,
    max_width: f32,
}

impl<'a> Chip<'a> {
    pub fn kind(kind: Kind, label: &'a str) -> Self {
        Chip {
            label,
            kind: Some(kind),
            selected: false,
            max_width: f32::INFINITY,
        }
    }

    pub fn plain(label: &'a str) -> Self {
        Chip {
            label,
            kind: None,
            selected: false,
            max_width: f32::INFINITY,
        }
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.selected = is_selected;
        self
    }

    /// Text past this width ends in "…". The tooltip and the accessible name keep all of it.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }
}

impl egui::Widget for Chip<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let tone = self.kind.map_or(Tone::Neutral, Kind::tone);
        let swatch = tone.swatch();
        let padding = 2.0 * space::SM;
        let mut job = egui::text::LayoutJob::single_section(
            self.label.to_owned(),
            TextRole::Small.format(color::TEXT),
        );
        job.wrap.max_rows = 1;
        job.wrap.overflow_character = Some('…');
        job.wrap.max_width = (self.max_width - padding).max(0.0);
        job.break_on_newline = false;
        let galley = ui.painter().layout_job(job);
        let size = egui::vec2(galley.size().x + padding, size::CHIP);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, self.label));
        if ui.is_rect_visible(rect) {
            let outline = if self.selected || response.hovered() {
                swatch.solid
            } else {
                swatch.edge
            };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, radius::MD, swatch.wash);
            painter.rect_stroke(
                rect,
                radius::MD,
                egui::Stroke::new(1.0, outline),
                egui::StrokeKind::Inside,
            );
            let at = egui::pos2(
                rect.left() + space::SM,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(at, galley.clone(), color::TEXT);
        }
        if galley.elided {
            response.on_hover_text(self.label)
        } else {
            response
        }
    }
}

/// A short status in a tone: "Copied", "Ingested".
pub struct Badge<'a> {
    text: &'a str,
    tone: Tone,
}

impl<'a> Badge<'a> {
    pub const fn new(text: &'a str) -> Self {
        Badge {
            text,
            tone: Tone::Neutral,
        }
    }

    pub const fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub const fn icon(self, _icon: Icon) -> Self {
        self
    }
}

impl egui::Widget for Badge<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.label(
            TextRole::Small
                .rich(self.text)
                .color(self.tone.swatch().text),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    Active,
    Pending,
}

/// A numbered circle of a step list. Its accessible name is "Step {n}".
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
        let label = format!("Step {}", self.number);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &label));
        if ui.is_rect_visible(rect) {
            let swatch = self.tone.swatch();
            let painter = ui.painter_at(rect);
            let radius = size::STEP / 2.0;
            let text = match self.state {
                StepState::Pending => {
                    painter.circle_stroke(
                        rect.center(),
                        radius,
                        egui::Stroke::new(1.0, color::BORDER),
                    );
                    color::TEXT_MUTED
                }
                StepState::Done | StepState::Active => {
                    painter.circle_filled(rect.center(), radius, swatch.solid);
                    swatch.on_solid
                }
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
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
