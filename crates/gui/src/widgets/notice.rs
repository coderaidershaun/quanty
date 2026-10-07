//! The message strip above content, and the card that fills an area that is empty, loading or
//! failed.

use eframe::egui::{
    self, Align, Align2, Layout, Response, Stroke, WidgetInfo, WidgetType, text::LayoutJob, vec2,
};

use super::{Button, ControlSize, spinner};
use crate::theme::{Icon, TextRole, Tone, color, radius, size, space, stroke};

/// A message strip above content: what happened, a line of detail, and what to do about it.
pub struct Notice<'a> {
    title: &'a str,
    tone: Tone,
    icon: Icon,
    body: Option<&'a str>,
    action: Option<&'a str>,
    is_dismissable: bool,
}

/// What a notice reports. `dismissed` is its close button.
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

    /// A button under the text. `label` is its name.
    pub fn action(mut self, label: &'a str) -> Self {
        self.action = Some(label);
        self
    }

    /// Adds a close button, named "Dismiss".
    pub fn dismissable(mut self) -> Self {
        self.is_dismissable = true;
        self
    }

    /// The notice as one node named "{title}. {body}", or the title alone.
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

/// A placeholder is at least this high, so it stays readable in a small panel.
const PLACEHOLDER_MIN_HEIGHT: f32 = 120.0;
/// The hint under a placeholder's title is never wider than this.
const PLACEHOLDER_TEXT_WIDTH: f32 = 360.0;
/// The room for the icon, or the spinner, above the title.
const PLACEHOLDER_ICON: f32 = 32.0;

/// What fills a whole area that is empty, loading or failed. It is centred in the room it is given.
pub struct Placeholder<'a> {
    fill: Fill,
    title: &'a str,
    hint: Option<&'a str>,
    action: Option<&'a str>,
}

/// What a placeholder reports. `action_clicked` is its button.
pub struct PlaceholderResponse {
    pub response: Response,
    pub action_clicked: bool,
}

impl<'a> Placeholder<'a> {
    fn with_fill(fill: Fill, title: &'a str) -> Self {
        Placeholder {
            fill,
            title,
            hint: None,
            action: None,
        }
    }

    pub fn empty(_icon: Icon, title: &'a str) -> Self {
        // SMELL: `_icon` is used, so its underscore is wrong. Rename it to `icon`.
        Placeholder::with_fill(Fill::Empty(_icon), title)
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

    /// The placeholder as one node named "{title}. {hint}", or the title alone.
    pub fn show(self, ui: &mut egui::Ui) -> PlaceholderResponse {
        let area = ui.available_size();
        let height = if area.y.is_finite() {
            area.y.max(PLACEHOLDER_MIN_HEIGHT)
        } else {
            PLACEHOLDER_MIN_HEIGHT
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
                self.paint_icon(ui);
                ui.scope(|ui| {
                    ui.set_max_width(text_width);
                    ui.label(self.title_text());
                    if let Some(hint) = self.hint {
                        ui.label(TextRole::Small.rich(hint));
                    }
                });
                if let Some(label) = self.action {
                    let button = Button::secondary(label).size(ControlSize::Small);
                    action_clicked = ui.add(button).clicked();
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

    /// The height of everything that is stacked in the middle, to centre it.
    fn content_height(&self, ui: &egui::Ui, text_width: f32) -> f32 {
        // SMELL: this lists again what `show` stacks. A part that is added there and not here
        // puts the placeholder off centre.
        let gap = ui.spacing().item_spacing.y;
        let mut heights = vec![
            PLACEHOLDER_ICON,
            text_height(ui, TextRole::BodyStrong, self.title, text_width),
        ];
        if let Some(hint) = self.hint {
            heights.push(text_height(ui, TextRole::Small, hint, text_width));
        }
        if self.action.is_some() {
            heights.push(ControlSize::Small.height());
        }
        heights.iter().sum::<f32>() + gap * (heights.len() - 1) as f32
    }
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

/// The height of `text` wrapped at `width`, laid out as a label of `role` lays it out.
fn text_height(ui: &egui::Ui, role: TextRole, text: &str, width: f32) -> f32 {
    let mut job = LayoutJob::single_section(text.to_owned(), role.format(color::TEXT));
    job.wrap.max_width = width;
    ui.painter().layout_job(job).size().y
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::state::Shared;
    use crate::testkit;
    use crate::widgets::Chip;

    #[test]
    fn a_notice_a_placeholder_and_a_cut_chip_are_named_by_their_whole_text() {
        let long = "A chip with a label far too long to fit";
        let mut harness = testkit::panel([400.0, 300.0], Shared::default(), move |ui, _cx| {
            Notice::warning("Qdrant is down")
                .body("Start it and try again.")
                .dismissable()
                .show(ui);
            Placeholder::loading("Loading the page")
                .hint("This takes a moment.")
                .show(ui);
            ui.add(Chip::plain(long).max_width(80.0));
        });
        // The spinner asks for a repaint after a delay, so `run` settles with it on screen.
        harness.run();
        harness.get_by_label("Qdrant is down. Start it and try again.");
        harness.get_by_label("Loading the page. This takes a moment.");
        harness.get_by_label("Dismiss");
        harness.get_by_label(long);
    }
}
