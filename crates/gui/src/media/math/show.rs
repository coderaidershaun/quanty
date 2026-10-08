//! The widget that draws one formula in each state it can be in, under one accessible name.

use eframe::egui::{
    self, Direction, Layout, Response, ScrollArea, Sense, Ui, UiBuilder, Vec2, WidgetInfo,
    WidgetType, pos2, vec2,
};

use super::image::{Display, MathImage, MathRef, MathState};
use crate::media::Media;
use crate::theme::{TextRole, color, radius};
use crate::widgets::spinner;

const BLOCK_WAITING_HEIGHT_EM: f32 = 2.4;
const INLINE_WAITING_SIZE_EM: Vec2 = vec2(2.0, 1.2);
/// A block formula is never drawn smaller than this. A wider one scrolls instead.
const SMALLEST_SCALE: f32 = 0.6;
const WAITING_NAME: &str = "Typesetting formula";

/// In every state there is exactly one accessible node, an `Image` named by the LaTeX source.
pub fn show(ui: &mut Ui, media: &mut Media, math: &MathRef<'_>) -> Response {
    let state = media.math.get(math);
    // A parent with no width limit has no width to centre in or to fit to, so a block is laid
    // out there as an inline formula is.
    let display = if ui.available_width().is_finite() {
        math.display
    } else {
        Display::Inline
    };
    let response = match state {
        MathState::Loading => waiting(ui, display, math.size),
        MathState::Ready(image) => match display {
            Display::Block => fitted(ui, &image, math.latex),
            Display::Inline => at_own_size(ui, &image),
        },
        MathState::Failed => source(ui, display, math.latex),
    };
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, math.latex));
    response
}

/// An inline formula holds a gap and shows no spinner, so that a line of text does not fill with
/// spinners.
fn waiting(ui: &mut Ui, display: Display, em: f32) -> Response {
    if display == Display::Inline {
        return ui.allocate_response(INLINE_WAITING_SIZE_EM * em, Sense::hover());
    }
    let size = vec2(ui.available_width(), BLOCK_WAITING_HEIGHT_EM * em);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, radius::MD, color::RAISED);
    let centred = UiBuilder::new()
        .max_rect(rect)
        .layout(Layout::centered_and_justified(Direction::TopDown));
    spinner(&mut ui.new_child(centred), WAITING_NAME);
    response
}

fn at_own_size(ui: &mut Ui, image: &MathImage) -> Response {
    let (rect, response) = ui.allocate_exact_size(image.size(), Sense::hover());
    let baseline_left = rect.left_top() + vec2(0.0, image.ascent);
    image.paint(ui.painter(), baseline_left, 1.0, color::TEXT);
    response
}

fn fitted(ui: &mut Ui, image: &MathImage, latex: &str) -> Response {
    let available = ui.available_width();
    let scale = (available / image.width).min(1.0);
    if scale < SMALLEST_SCALE {
        return scrolled(ui, image, latex);
    }
    let size = image.size() * scale;
    let (rect, response) = ui.allocate_exact_size(vec2(available, size.y), Sense::hover());
    let baseline_left = pos2(
        rect.center().x - size.x / 2.0,
        rect.top() + image.ascent * scale,
    );
    image.paint(ui.painter(), baseline_left, scale, color::TEXT);
    response
}

fn scrolled(ui: &mut Ui, image: &MathImage, latex: &str) -> Response {
    // SMELL: two equal formulas in one parent share this id, so they scroll together.
    let scrolling = ScrollArea::horizontal().id_salt(egui::util::hash(latex));
    scrolling
        .show(ui, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(image.size() * SMALLEST_SCALE, Sense::hover());
            let baseline_left = rect.left_top() + vec2(0.0, image.ascent * SMALLEST_SCALE);
            image.paint(ui.painter(), baseline_left, SMALLEST_SCALE, color::TEXT);
            response
        })
        .inner
}

/// The LaTeX is painted, not a label, so that the only node that carries the source as its name
/// is the formula's own.
fn source(ui: &mut Ui, display: Display, latex: &str) -> Response {
    let font = TextRole::Mono.font();
    let galley = match display {
        Display::Block => {
            let text = latex.to_owned();
            ui.painter()
                .layout(text, font, color::TEXT_MUTED, ui.available_width())
        }
        Display::Inline => {
            let one_line = latex.replace(['\n', '\r'], " ");
            ui.painter()
                .layout_no_wrap(one_line, font, color::TEXT_MUTED)
        }
    };
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color::TEXT_MUTED);
    response
}
