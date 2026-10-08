//! The page picture in a viewport of its own: it takes the drag, the wheel, the pinch and the
//! zoom keys, keeps the page in the viewport, and paints the page with the cited figure framed.

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, Key, Modifiers, Sense, Stroke, StrokeKind, Vec2,
    WidgetInfo, WidgetType,
};

use super::view::{self, View, Viewport, ZoomStep};
use super::zoom_bar::ZoomAction;
use crate::contract::PageBox;
use crate::media::images::Picture;
use crate::theme::{Tone, color, radius, stroke};

#[derive(Debug, Clone, Copy)]
pub(super) struct Figure<'a> {
    pub cut: PageBox,
    pub name: &'a str,
    pub caption: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Shown<'a> {
    pub picture: Picture,
    pub page_number: u32,
    pub figure: Option<Figure<'a>>,
    pub should_reveal: bool,
}

pub(super) fn show(ui: &mut egui::Ui, view: &mut View, shown: &Shown<'_>) {
    let (area, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
    response.widget_info(|| {
        let name = format!("Picture of page {}", shown.page_number);
        WidgetInfo::labeled(WidgetType::Image, true, name)
    });
    let viewport = Viewport {
        rect: area,
        picture: shown.picture.size(),
    };
    if response.dragged() {
        view.pan -= response.drag_delta();
    }
    if response.contains_pointer() {
        take_input(ui, &viewport, view);
    }
    if let (true, Some(figure)) = (shown.should_reveal, shown.figure) {
        *view = viewport.reveal(*view, figure.cut);
    }
    *view = viewport.clamp(*view);
    let page = viewport.page_rect(*view);
    if page.width() > area.width() || page.height() > area.height() {
        let cursor = if response.dragged() {
            CursorIcon::Grabbing
        } else {
            CursorIcon::Grab
        };
        response.clone().on_hover_cursor(cursor);
    }

    let painter = ui.painter_at(area);
    painter.rect_filled(area, CornerRadius::ZERO, color::CANVAS);
    painter.rect_filled(page, CornerRadius::ZERO, color::PAGE);
    shown.picture.paint(&painter, page, Color32::WHITE);
    if let Some(figure) = shown.figure {
        let seen = view::box_rect(page, figure.cut).intersect(area);
        if seen.is_positive() {
            painter.rect_filled(seen, radius::SM, color::PAGE_HIGHLIGHT);
            let outline = Stroke::new(stroke::UNDERLINE, Tone::Blue.swatch().solid);
            painter.rect_stroke(seen, radius::SM, outline, StrokeKind::Inside);
            let mark = ui.interact(seen, response.id.with("highlight"), Sense::hover());
            mark.widget_info(|| {
                let name = format!("{} on the page", figure.name);
                WidgetInfo::labeled(WidgetType::Label, true, name)
            });
            if let Some(caption) = figure.caption {
                mark.on_hover_text(caption);
            }
        }
    }
}

fn take_input(ui: &egui::Ui, viewport: &Viewport, view: &mut View) {
    let (scroll, pinch, pointer) = ui.input(|input| {
        (
            input.smooth_scroll_delta(),
            input.zoom_delta(),
            input.pointer.hover_pos(),
        )
    });
    if scroll != Vec2::ZERO {
        view.pan -= scroll;
        // Nothing else under the pointer should scroll as well.
        ui.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
    }
    if pinch != 1.0 {
        let anchor = pointer.unwrap_or(viewport.rect.center());
        *view = viewport.zoom_about(*view, anchor, view.zoom * pinch);
    }
    if let Some(action) = key_zoom(ui) {
        *view = action.apply(viewport, *view);
    }
}

/// The zoom key that was pressed, taken so that the window does not zoom as well.
fn key_zoom(ui: &egui::Ui) -> Option<ZoomAction> {
    ui.input_mut(|input| {
        let command = Modifiers::COMMAND;
        if input.consume_key(command, Key::Plus) || input.consume_key(command, Key::Equals) {
            Some(ZoomAction::Step(ZoomStep::In))
        } else if input.consume_key(command, Key::Minus) {
            Some(ZoomAction::Step(ZoomStep::Out))
        } else if input.consume_key(command, Key::Num0) {
            Some(ZoomAction::Fit)
        } else {
            None
        }
    })
}
