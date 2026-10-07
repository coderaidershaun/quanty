//! The message strip above content, and the card that fills an area that is empty, loading or
//! failed.

use eframe::egui::{self, Response, WidgetInfo, WidgetType};

use super::{Button, spinner};
use crate::theme::{Icon, TextRole, Tone, color, radius, space};

pub struct Notice<'a> {
    title: &'a str,
    tone: Tone,
    body: Option<&'a str>,
    action: Option<&'a str>,
    dismissable: bool,
}

/// What a notice reports. `dismissed` is its close button.
pub struct NoticeResponse {
    pub response: Response,
    pub action_clicked: bool,
    pub dismissed: bool,
}

impl<'a> Notice<'a> {
    fn with_tone(title: &'a str, tone: Tone) -> Self {
        Notice {
            title,
            tone,
            body: None,
            action: None,
            dismissable: false,
        }
    }

    pub fn info(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Blue)
    }

    pub fn success(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Success)
    }

    pub fn warning(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Warning)
    }

    pub fn error(title: &'a str) -> Self {
        Notice::with_tone(title, Tone::Danger)
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
        self.dismissable = true;
        self
    }

    /// The notice as one node named "{title}. {body}", or the title alone.
    pub fn show(self, ui: &mut egui::Ui) -> NoticeResponse {
        let swatch = self.tone.swatch();
        let mut action_clicked = false;
        let mut dismissed = false;
        let framed = egui::Frame::NONE
            .fill(swatch.wash)
            .stroke(egui::Stroke::new(1.0, swatch.edge))
            .corner_radius(radius::LG)
            .inner_margin(space::LG)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(TextRole::BodyStrong.rich(self.title).color(swatch.text));
                    if self.dismissable {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            dismissed = ui.add(Button::icon_only(Icon::CLOSE, "Dismiss")).clicked();
                        });
                    }
                });
                if let Some(body) = self.body {
                    ui.label(TextRole::Body.rich(body).color(color::TEXT_SECONDARY));
                }
                if let Some(label) = self.action {
                    action_clicked = ui.add(Button::secondary(label)).clicked();
                }
            });
        let name = match self.body {
            Some(body) => format!("{}. {body}", self.title),
            None => self.title.to_owned(),
        };
        framed
            .response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &name));
        NoticeResponse {
            response: framed.response,
            action_clicked,
            dismissed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fill {
    Empty,
    Loading,
    Error,
}

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
        Placeholder::with_fill(Fill::Empty, title)
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
        let mut action_clicked = false;
        let centred = ui.vertical_centered(|ui| {
            ui.add_space(space::XL);
            if self.fill == Fill::Loading {
                spinner(ui, self.title);
            }
            let title_color = if self.fill == Fill::Error {
                Tone::Danger.swatch().text
            } else {
                color::TEXT
            };
            ui.label(TextRole::BodyStrong.rich(self.title).color(title_color));
            if let Some(hint) = self.hint {
                ui.label(TextRole::Small.rich(hint));
            }
            if let Some(label) = self.action {
                action_clicked = ui.add(Button::secondary(label)).clicked();
            }
            ui.add_space(space::XL);
        });
        let name = match self.hint {
            Some(hint) => format!("{}. {hint}", self.title),
            None => self.title.to_owned(),
        };
        centred
            .response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &name));
        PlaceholderResponse {
            response: centred.response,
            action_clicked,
        }
    }
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
