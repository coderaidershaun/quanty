//! The message strip above content, and the message that fills an area that is empty, loading or
//! failed.

use eframe::egui::{
    self, Align, Layout, Response, Stroke, WidgetInfo, WidgetType, text::LayoutJob, vec2,
};

use crate::theme::{self, Icon, TextRole, Tone, radius, size, space, stroke};
use crate::widgets::controls::Button;
use crate::widgets::{ControlSize, spinner};

pub struct Notice<'a> {
    title: &'a str,
    tone: Tone,
    icon: Icon,
    body: Option<&'a str>,
    action: Option<&'a str>,
    control_size: ControlSize,
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
            control_size: ControlSize::Small,
            is_dismissable: false,
        }
    }

    pub fn info(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Indigo, Icon::INFO)
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

    /// The size of the action and of the dismiss button, `Small` unless a finger presses them.
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.control_size = size;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> NoticeResponse {
        let swatch = theme::palette(ui.ctx()).tone(self.tone);
        let mut action_clicked = false;
        let mut dismissed = false;
        let framed = egui::Frame::NONE
            .fill(swatch.wash)
            .stroke(Stroke::new(stroke::BORDER, swatch.edge))
            .corner_radius(radius::SM)
            .inner_margin(space::LG)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let (_, icon_rect) = ui.allocate_space(egui::Vec2::splat(size::ICON_LG));
                    self.icon
                        .paint(ui.painter(), icon_rect.center(), size::ICON_LG, swatch.text);
                    let after = if self.is_dismissable {
                        self.control_size.height() + ui.spacing().item_spacing.x
                    } else {
                        0.0
                    };
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - after).max(0.0));
                        ui.label(TextRole::BodyStrong.rich(ui, self.title).color(swatch.text));
                        if let Some(body) = self.body {
                            ui.label(TextRole::Body.rich(ui, body).color(swatch.text));
                        }
                        if let Some(label) = self.action {
                            let button = Button::secondary(label).size(self.control_size);
                            action_clicked = ui.add(button).clicked();
                        }
                    });
                    if self.is_dismissable {
                        let dismiss =
                            Button::icon_only(Icon::CLOSE, "Dismiss").size(self.control_size);
                        dismissed = ui.add(dismiss).clicked();
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
const PLACEHOLDER_ICON: f32 = 28.0;

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
    action_size: ControlSize,
}

pub struct PlaceholderResponse {
    pub response: Response,
    pub action_clicked: bool,
}

impl<'a> Placeholder<'a> {
    /// A placeholder with its icon is at least this high, so it stays readable in a small panel.
    /// A room too low for the whole stack gets the words alone, so none of them is cut off.
    pub const MIN_HEIGHT: f32 = 120.0;

    fn with_fill(fill: Fill, title: &'a str) -> Self {
        Placeholder {
            fill,
            title,
            hint: None,
            action: None,
            action_size: ControlSize::Small,
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

    /// The size of the action button, `Small` unless a finger presses it.
    pub fn action_size(mut self, size: ControlSize) -> Self {
        self.action_size = size;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> PlaceholderResponse {
        let area = ui.available_size();
        let width = if area.x.is_finite() {
            area.x
        } else {
            PLACEHOLDER_TEXT_WIDTH
        };
        let text_width = width.min(PLACEHOLDER_TEXT_WIDTH);
        // A spinner says that work is on its way, so it stays.
        let has_icon = !area.y.is_finite()
            || self.fill == Fill::Loading
            || area.y >= self.content_height(ui, text_width, true);
        let height = match (area.y.is_finite(), has_icon) {
            (false, _) => Self::MIN_HEIGHT,
            (true, true) => area.y.max(Self::MIN_HEIGHT),
            (true, false) => area.y.max(self.content_height(ui, text_width, false)),
        };
        let top = ((height - self.content_height(ui, text_width, has_icon)) / 2.0).max(0.0);
        let mut action_clicked = false;
        let centred = ui.allocate_ui_with_layout(
            vec2(width, height),
            Layout::top_down(Align::Center),
            |ui| {
                ui.add_space(top);
                for part in self.parts(has_icon) {
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

    fn parts(&self, has_icon: bool) -> Vec<Part<'a>> {
        let icon = has_icon.then_some(Part::Icon);
        let hint = self.hint.map(Part::Hint);
        let action = self.action.map(Part::Action);
        [icon, Some(Part::Title), hint, action]
            .into_iter()
            .flatten()
            .collect()
    }

    /// Returns true when the part is the action and it was clicked.
    fn show_part(&self, ui: &mut egui::Ui, part: Part<'_>, text_width: f32) -> bool {
        let mut clicked = false;
        match part {
            Part::Icon => self.paint_icon(ui),
            Part::Title => wrapped_label(ui, self.title_text(ui), text_width),
            Part::Hint(hint) => wrapped_label(ui, TextRole::Small.rich(ui, hint), text_width),
            Part::Action(label) => {
                let button = Button::secondary(label).size(self.action_size);
                clicked = ui.add(button).clicked();
            }
        }
        clicked
    }

    fn height_of(&self, ui: &egui::Ui, part: Part<'_>, text_width: f32) -> f32 {
        match part {
            Part::Icon => PLACEHOLDER_ICON,
            Part::Title => text_height(ui, TextRole::Heading, self.title, text_width),
            Part::Hint(hint) => text_height(ui, TextRole::Small, hint, text_width),
            Part::Action(_) => self.action_size.height(),
        }
    }

    fn title_text(&self, ui: &egui::Ui) -> egui::RichText {
        let palette = theme::palette(ui.ctx());
        let tint = if self.fill == Fill::Error {
            palette.tone(Tone::Danger).text
        } else {
            TextRole::Heading.color(palette)
        };
        TextRole::Heading.rich(ui, self.title).color(tint)
    }

    fn paint_icon(&self, ui: &mut egui::Ui) {
        let palette = theme::palette(ui.ctx());
        match self.fill {
            Fill::Loading => {
                ui.add_space((PLACEHOLDER_ICON - size::ICON_MD) / 2.0);
                spinner(ui, self.title);
                ui.add_space((PLACEHOLDER_ICON - size::ICON_MD) / 2.0);
            }
            Fill::Empty(icon) => paint_big_icon(ui, icon, palette.text_faint),
            Fill::Error => paint_big_icon(ui, Icon::ERROR, palette.tone(Tone::Danger).text),
        }
    }

    fn content_height(&self, ui: &egui::Ui, text_width: f32, has_icon: bool) -> f32 {
        let gap = ui.spacing().item_spacing.y;
        let parts = self.parts(has_icon);
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
    icon.paint(ui.painter(), rect.center(), PLACEHOLDER_ICON, tint);
}

fn text_height(ui: &egui::Ui, role: TextRole, text: &str, width: f32) -> f32 {
    let mut job =
        LayoutJob::single_section(text.to_owned(), role.format(egui::Color32::PLACEHOLDER));
    job.wrap.max_width = width;
    ui.painter().layout_job(job).size().y
}
