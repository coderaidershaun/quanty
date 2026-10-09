//! The message strip above content, and the card that fills an area that is empty, loading or
//! failed.

use eframe::egui::{
    self, Align, Align2, Layout, Response, Stroke, WidgetInfo, WidgetType, text::LayoutJob, vec2,
};

use super::{Button, ControlSize, spinner};
use crate::theme::{Icon, TextRole, Tone, color, radius, size, space, stroke};

pub struct Notice<'a> {
    title: &'a str,
    tone: Tone,
    icon: Icon,
    body: Option<&'a str>,
    action: Option<&'a str>,
    is_dismissable: bool,
}

pub struct NoticeResponse {
    pub response: Response,
    pub action_clicked: bool,
    pub dismissed: bool,
}

impl<'a> Notice<'a> {
    fn with_tone(title: &'a str, tone: Tone, icon: Icon) -> Self {
        Notice {
            title,
            tone,
            icon,
            body: None,
            action: None,
            is_dismissable: false,
        }
    }

    pub fn info(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Blue, Icon::INFO)
    }

    pub fn success(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Success, Icon::SUCCESS)
    }

    pub fn warning(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Warning, Icon::WARNING)
    }

    pub fn error(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Danger, Icon::ERROR)
    }

    pub fn body(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self
    }

    pub fn action(mut self, label: &'a str) -> Self {
        self.action = Some(label);
        self
    }

    pub fn dismissable(mut self) -> Self {
        self.is_dismissable = true;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> NoticeResponse {
        let swatch = self.tone.swatch();
        let mut action_clicked = false;
        let mut dismissed = false;
        let framed = egui::Frame::NONE
            .fill(swatch.wash)
            .stroke(Stroke::new(stroke::BORDER, swatch.edge))
            .corner_radius(radius::LG)
            .inner_margin(space::LG)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let (_, icon_rect) = ui.allocate_space(egui::Vec2::splat(size::ICON_LG));
                    ui.painter().text(
                        icon_rect.center(),
                        Align2::CENTER_CENTER,
                        self.icon.glyph(),
                        Icon::font(size::ICON_LG),
                        swatch.text,
                    );
                    let after = if self.is_dismissable {
                        size::CONTROL_SM + ui.spacing().item_spacing.x
                    } else {
                        0.0
                    };
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - after).max(0.0));
                        ui.label(TextRole::BodyStrong.rich(self.title).color(swatch.text));
                        if let Some(body) = self.body {
                            ui.label(TextRole::Body.rich(body).color(color::TEXT_SECONDARY));
                        }
                        if let Some(label) = self.action {
                            let button = Button::secondary(label).size(ControlSize::Small);
                            action_clicked = ui.add(button).clicked();
                        }
                    });
                    if self.is_dismissable {
                        dismissed = ui.add(Button::icon_only(Icon::CLOSE, "Dismiss")).clicked();
                    }
                });
            });
        framed.response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::Label, true, whole_text(self.title, self.body))
        });
        NoticeResponse {
            response: framed.response,
            action_clicked,
            dismissed,
        }
    }
}

/// The name a notice or a placeholder answers to as a whole: "{title}. {detail}", or the title
/// alone.
fn whole_text(title: &str, detail: Option<&str>) -> String {
    match detail {
        Some(detail) => format!("{title}. {detail}"),
        None => title.to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fill {
    Empty(Icon),
    Loading,
    Error,
}

const PLACEHOLDER_TEXT_WIDTH: f32 = 360.0;
const PLACEHOLDER_ICON: f32 = 32.0;

/// What a placeholder stacks, from the top. The drawing and the measure that centres the stack
/// both walk the same list, so a new part cannot be left out of one of them.
#[derive(Debug, Clone, Copy)]
enum Part<'a> {
    Icon,
    Title,
    Hint(&'a str),
    Action(&'a str),
}

pub struct Placeholder<'a> {
    fill: Fill,
    title: &'a str,
    hint: Option<&'a str>,
    action: Option<&'a str>,
}

pub struct PlaceholderResponse {
    pub response: Response,
    pub action_clicked: bool,
}

impl<'a> Placeholder<'a> {
    /// A placeholder is at least this high, so it stays readable in a small panel.
    pub const MIN_HEIGHT: f32 = 120.0;

    fn with_fill(fill: Fill, title: &'a str) -> Self {
        Placeholder {
            fill,
            title,
            hint: None,
            action: None,
        }
    }

    pub fn empty(icon: Icon, title: &'a str) -> Self {
        Placeholder::with_fill(Fill::Empty(icon), title)
    }

    pub fn loading(title: &'a str) -> Self {
        Placeholder::with_fill(Fill::Loading, title)
    }

    pub fn error(title: &'a str) -> Self {
        Placeholder::with_fill(Fill::Error, title)
    }

    pub fn hint(mut self, text: &'a str) -> Self {
        self.hint = Some(text);
        self
    }

    pub fn action(mut self, label: &'a str) -> Self {
        self.action = Some(label);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> PlaceholderResponse {
        let area = ui.available_size();
        let height = if area.y.is_finite() {
            area.y.max(Self::MIN_HEIGHT)
        } else {
            Self::MIN_HEIGHT
        };
        let width = if area.x.is_finite() {
            area.x
        } else {
            PLACEHOLDER_TEXT_WIDTH
        };
        let text_width = width.min(PLACEHOLDER_TEXT_WIDTH);
        let top = ((height - self.content_height(ui, text_width)) / 2.0).max(0.0);
        let mut action_clicked = false;
        let centred = ui.allocate_ui_with_layout(
            vec2(width, height),
            Layout::top_down(Align::Center),
            |ui| {
                ui.add_space(top);
                for part in self.parts() {
                    action_clicked |= self.show_part(ui, part, text_width);
                }
            },
        );
        centred.response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::Label, true, whole_text(self.title, self.hint))
        });
        PlaceholderResponse {
            response: centred.response,
            action_clicked,
        }
    }

    fn parts(&self) -> Vec<Part<'a>> {
        let hint = self.hint.map(Part::Hint);
        let action = self.action.map(Part::Action);
        [Some(Part::Icon), Some(Part::Title), hint, action]
            .into_iter()
            .flatten()
            .collect()
    }

    /// Returns true when the part is the action and it was clicked.
    fn show_part(&self, ui: &mut egui::Ui, part: Part<'_>, text_width: f32) -> bool {
        let mut clicked = false;
        match part {
            Part::Icon => self.paint_icon(ui),
            Part::Title => wrapped_label(ui, self.title_text(), text_width),
            Part::Hint(hint) => wrapped_label(ui, TextRole::Small.rich(hint), text_width),
            Part::Action(label) => {
                let button = Button::secondary(label).size(ControlSize::Small);
                clicked = ui.add(button).clicked();
            }
        }
        clicked
    }

    fn height_of(&self, ui: &egui::Ui, part: Part<'_>, text_width: f32) -> f32 {
        match part {
            Part::Icon => PLACEHOLDER_ICON,
            Part::Title => text_height(ui, TextRole::BodyStrong, self.title, text_width),
            Part::Hint(hint) => text_height(ui, TextRole::Small, hint, text_width),
            Part::Action(_) => ControlSize::Small.height(),
        }
    }

    fn title_text(&self) -> egui::RichText {
        let tint = if self.fill == Fill::Error {
            Tone::Danger.swatch().text
        } else {
            color::TEXT
        };
        TextRole::BodyStrong.rich(self.title).color(tint)
    }

    fn paint_icon(&self, ui: &mut egui::Ui) {
        match self.fill {
            Fill::Loading => {
                ui.add_space((PLACEHOLDER_ICON - size::ICON_MD) / 2.0);
                spinner(ui, self.title);
                ui.add_space((PLACEHOLDER_ICON - size::ICON_MD) / 2.0);
            }
            Fill::Empty(icon) => paint_big_icon(ui, icon, color::TEXT_MUTED),
            Fill::Error => paint_big_icon(ui, Icon::ERROR, Tone::Danger.swatch().text),
        }
    }

    fn content_height(&self, ui: &egui::Ui, text_width: f32) -> f32 {
        let gap = ui.spacing().item_spacing.y;
        let parts = self.parts();
        let stacked: f32 = parts
            .iter()
            .map(|part| self.height_of(ui, *part, text_width))
            .sum();
        stacked + gap * (parts.len() - 1) as f32
    }
}

fn wrapped_label(ui: &mut egui::Ui, text: egui::RichText, width: f32) {
    ui.scope(|ui| {
        ui.set_max_width(width);
        ui.label(text);
    });
}

fn paint_big_icon(ui: &mut egui::Ui, icon: Icon, tint: egui::Color32) {
    let (_, rect) = ui.allocate_space(egui::Vec2::splat(PLACEHOLDER_ICON));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.glyph(),
        Icon::font(PLACEHOLDER_ICON),
        tint,
    );
}

fn text_height(ui: &egui::Ui, role: TextRole, text: &str, width: f32) -> f32 {
    let mut job = LayoutJob::single_section(text.to_owned(), role.format(color::TEXT));
    job.wrap.max_width = width;
    ui.painter().layout_job(job).size().y
}
