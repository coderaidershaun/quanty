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

/// A figure of the page that a result cites, to frame.
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
    /// Bring the figure into view in this frame.
    pub should_reveal: bool,
}

/// Draws the page in all the room the ui has.
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
        let frame = view::box_rect(page, figure.cut);
        painter.rect_filled(frame, radius::SM, color::PAGE_HIGHLIGHT);
        let outline = Stroke::new(stroke::UNDERLINE, Tone::Blue.swatch().solid);
        painter.rect_stroke(frame, radius::SM, outline, StrokeKind::Inside);
        let seen = frame.intersect(area);
        if seen.is_positive() {
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

/// The wheel, the pinch and the zoom keys, while the pointer is over the page.
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

#[cfg(test)]
mod tests {
    use eframe::egui::accesskit::Role;
    use eframe::egui::{Key, Modifiers, Rect};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use crate::contract::{Intent, PieceKind};
    use crate::panels::source::samples::{
        self, DEFAULT_SIZE as DEFAULT, SMALLEST_SIZE as SMALLEST, Sample, has_picture_of,
        picture_name, place_text, shows,
    };
    use crate::testkit::{self, Host};

    /// Low enough for the header to fold to two rows, and wide enough for the pager to fit
    /// beside the title whatever its buttons look like.
    const LOW_AND_WIDE: [f32; 2] = [760.0, 300.0];

    /// True when the node is inside the panel from top to bottom.
    fn is_in_view(size: [f32; 2], node: Rect) -> bool {
        let panel = samples::panel_rect(size);
        panel.top() <= node.top() && node.bottom() <= panel.bottom()
    }

    fn shares_a_row(one: Rect, other: Rect) -> bool {
        one.top() < other.bottom() && other.top() < one.bottom()
    }

    fn picture_of(harness: &Harness<'static, Host>, sample: Sample) -> Rect {
        let name = picture_name(sample);
        harness.get_by_role_and_label(Role::Image, &name).rect()
    }

    /// True when the pager of the page of `Sample::FigureTop` ends where the page picture ends:
    /// at the right edge.
    fn pager_is_at_the_right_edge(harness: &Harness<'static, Host>) -> bool {
        let edge = picture_of(harness, Sample::FigureTop).right();
        let place = place_text(&samples::page(Sample::FigureTop));
        let count = harness.get_by_label(&place).rect();
        (count.right() - edge).abs() <= 1.0
    }

    #[test]
    fn the_page_frames_its_figure_keeps_its_zoom_over_a_turn_and_has_the_next_page_ready() {
        let cited = Sample::FigureLow.piece(PieceKind::Figure);
        let name = cited
            .label
            .as_deref()
            .expect("the sample figure has a label");
        let open = samples::opened(Sample::FigureLow, Some(cited.number));
        let mut harness = samples::panel(DEFAULT, open);
        samples::see_pictures(&mut harness);
        let page = picture_of(&harness, Sample::FigureLow);
        let figure = harness.get_by_label(&format!("{name} on the page")).rect();
        assert!(
            page.expand(0.5).contains_rect(figure),
            "{figure:?} in {page:?}"
        );
        assert!(figure.height() > 100.0, "{figure:?}");
        assert!(is_in_view(DEFAULT, figure));
        testkit::save_png(&mut harness, "source-figure");

        harness.get_by_label("Zoom in").click();
        harness.run();
        assert!(shows(&harness, "110%"));

        harness.get_by_label("Previous page").click();
        harness.run();
        assert_eq!(harness.state().intents, vec![Intent::TurnPage(-1)]);
        harness.state_mut().apply_intents();
        harness.run();
        assert!(
            has_picture_of(&harness, Sample::FigureLow),
            "the old page stays while the next loads"
        );
        samples::land(&mut harness.state_mut().shared, Sample::EveryKind);
        harness.run();
        assert!(
            has_picture_of(&harness, Sample::EveryKind),
            "the picture was ready before the turn"
        );
        assert!(!has_picture_of(&harness, Sample::FigureLow));
        assert!(shows(&harness, "110%"), "a turn keeps the zoom");

        harness.get_by_label("Zoom in").click();
        harness.run();
        harness.get_by_label("Zoom in").click();
        harness.run();
        assert!(shows(&harness, "150%"));
        testkit::save_png(&mut harness, "source-zoomed");

        harness
            .get_by_role_and_label(Role::Image, &picture_name(Sample::EveryKind))
            .hover();
        harness.run_ok();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Num0);
        harness.run();
        assert!(
            shows(&harness, "100%"),
            "the key fits the page to the width"
        );
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Equals);
        harness.run();
        assert!(shows(&harness, "110%"));
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Minus);
        harness.run();
        assert!(shows(&harness, "100%"));
        harness.get_by_label("Zoom in").click();
        harness.run();

        let doc_open = |sample: Sample| Intent::OpenSource {
            doc: sample.doc(),
            page: sample.page_number(),
            piece: None,
        };
        for sample in [Sample::Text, Sample::EveryKind] {
            let shared = &mut harness.state_mut().shared;
            shared.apply_intent(doc_open(sample), &mut Vec::new());
            samples::land(shared, sample);
            harness.run();
        }
        assert!(has_picture_of(&harness, Sample::EveryKind));
        assert!(shows(&harness, "100%"), "another document starts at fit");

        the_page_keeps_its_room_under_a_header_of_two_or_three_rows();
    }

    /// The last part of the test above: the page and the header over it at the default size, in
    /// a low and wide panel, and at the smallest size.
    fn the_page_keeps_its_room_under_a_header_of_two_or_three_rows() {
        let mut harness = samples::panel(DEFAULT, samples::opened(Sample::FigureTop, None));
        samples::see_pictures(&mut harness);
        assert!(has_picture_of(&harness, Sample::FigureTop));
        let next = harness.get_by_label("Next page").rect();
        let chapter = harness.get_by_label("Chapter").rect();
        assert!(
            shares_a_row(next, chapter),
            "three rows: the pager shares a row with the chapter"
        );
        assert!(
            pager_is_at_the_right_edge(&harness),
            "three rows: the chapter picker takes the room the pager leaves"
        );
        testkit::save_png(&mut harness, "source-page");

        let mut harness = samples::panel(LOW_AND_WIDE, samples::opened(Sample::FigureTop, None));
        samples::see_pictures(&mut harness);
        assert!(
            pager_is_at_the_right_edge(&harness),
            "two rows: the pager is at the right of the title's row"
        );
        let previous = harness.get_by_label("Previous page").rect();
        let next = harness.get_by_label("Next page").rect();
        assert!(
            previous.right() <= next.left(),
            "the pager reads from the left"
        );

        let mut harness = samples::panel(SMALLEST, samples::opened(Sample::FigureTop, None));
        samples::see_pictures(&mut harness);
        let page = picture_of(&harness, Sample::FigureTop);
        assert!(page.height() > 60.0, "the page has room: {page:?}");
        assert!(is_in_view(SMALLEST, page));
        let next = harness.get_by_label("Next page").rect();
        let book = harness.get_by_label("Book").rect();
        let chapter = harness.get_by_label("Chapter").rect();
        assert!(
            next.bottom() <= book.top(),
            "two rows: the pager is in the title's row"
        );
        assert!(
            shares_a_row(book, chapter),
            "two rows: the pickers share a row"
        );
        testkit::save_png(&mut harness, "source-small");
    }
}
