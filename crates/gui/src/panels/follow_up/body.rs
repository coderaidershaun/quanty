//! Draws what shows under the box: one message, then the questions to ask next.

use eframe::egui::text::LayoutJob;
use eframe::egui::{self, Color32, Sense, StrokeKind, WidgetInfo, WidgetType};

use super::guidance::{self, Guidance, Message};
use crate::theme::{TextRole, color, hairline, radius, size, space, stroke};
use crate::widgets;

/// A panel narrower than this, as on a phone, wraps each suggested question to at most three
/// lines and makes each row tall enough for a finger. The narrowest wide window gives the panel
/// 388 points, so a wide window never wraps.
const NARROW: f32 = 380.0;

/// How a suggested question is set in its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fit {
    OneLine,
    Wrapped,
}

impl Fit {
    fn of(width: f32) -> Fit {
        if width < NARROW {
            Fit::Wrapped
        } else {
            Fit::OneLine
        }
    }
}

pub(super) fn show<'a>(ui: &mut egui::Ui, guidance: &Guidance<'a>) -> Option<&'a str> {
    if let Some(message) = &guidance.message {
        show_message(ui, message);
    }
    show_questions(ui, guidance.suggestions)
}

fn show_questions<'a>(ui: &mut egui::Ui, questions: &'a [String]) -> Option<&'a str> {
    let mut clicked = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (turn, question) in questions.iter().enumerate() {
            if question_row(ui, question, turn > 0).clicked() {
                clicked = Some(question.as_str());
            }
        }
    });
    clicked
}

// SMELL: the kit has no list row, so this row paints by hand the cut text, the hover fill and
// the focus ring that a kit widget would paint. A change to the kit's look will miss this row.
fn question_row(ui: &mut egui::Ui, question: &str, has_row_above: bool) -> egui::Response {
    let width = ui.available_width();
    let fit = Fit::of(width);
    let galley = ui
        .painter()
        .layout_job(question_job(question, width - 2.0 * space::SM, fit));
    let height = match fit {
        Fit::OneLine => size::CHIP,
        Fit::Wrapped => (galley.size().y + 2.0 * space::XS).max(size::CONTROL_MD),
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    crate::widgets::interactive_cursor(ui, &response);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), question));
    let is_hovered = response.hovered();
    let ink = if is_hovered {
        color::TEXT
    } else {
        TextRole::Caption.color()
    };
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if response.is_pointer_button_down_on() {
            painter.rect_filled(rect, radius::SM, color::WELL_DEEP);
        } else if is_hovered {
            painter.rect_filled(rect, radius::SM, color::WELL);
        }
        if has_row_above {
            let line = hairline(painter, color::HAIRLINE);
            painter.hline(rect.x_range(), rect.top() + line.width / 2.0, line);
        }
        if response.has_focus() {
            let ring = egui::Stroke::new(stroke::FOCUS, color::FOCUS);
            painter.rect_stroke(rect, radius::SM, ring, StrokeKind::Inside);
        }
        let top = rect.center().y - galley.size().y / 2.0;
        painter.galley(
            egui::pos2(rect.left() + space::SM, top),
            galley.clone(),
            ink,
        );
    }
    if galley.elided {
        response.on_hover_text(question)
    } else {
        response
    }
}

/// The colour is left to be chosen when the text is painted, because it depends on the pointer.
fn question_job(question: &str, width: f32, fit: Fit) -> LayoutJob {
    let ink = Color32::PLACEHOLDER;
    let mut job = LayoutJob::single_section(question.to_owned(), TextRole::Caption.format(ink));
    job.wrap.max_rows = match fit {
        Fit::OneLine => 1,
        Fit::Wrapped => 3,
    };
    job.wrap.overflow_character = Some('…');
    job.wrap.max_width = width.max(0.0);
    job.break_on_newline = false;
    job
}

fn show_message(ui: &mut egui::Ui, message: &Message<'_>) {
    match message {
        Message::Waiting(text) => {
            ui.horizontal(|ui| {
                widgets::spinner(ui, guidance::WAITING_LABEL);
                ui.label(TextRole::Small.rich(*text));
            });
        }
        Message::Line { text, hint } => {
            ui.label(TextRole::Small.rich(*text));
            if let Some(hint) = hint {
                ui.label(TextRole::Small.rich(*hint));
            }
        }
        Message::Notice { title, body } => {
            widgets::Notice::info(title).body(body).show(ui);
        }
    }
}
