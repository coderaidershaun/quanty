//! Draws a laid-out block: its text runs, the fill behind code, the formulas and the chips, with
//! its accessible name and its copy menu. It also tells when a layout no longer fits its formulas.

use eframe::egui::{
    Color32, Painter, Pos2, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, vec2,
};

use super::atom::Seen;
use super::flow::Flow;
use super::{Clicked, RichText};
use crate::media::math::{Math, MathState};
use crate::widgets::{Button, CitationChip};

/// The part of the block that can be seen, in the coordinates of the block. The block has no
/// place yet, so it is taken to start where the next widget goes.
pub(super) fn view_of(ui: &Ui) -> Rect {
    ui.clip_rect()
        .translate(-ui.next_widget_position().to_vec2())
}

/// Whether the layout no longer matches the formulas that it shows. A layout that waits is
/// stale when none of its formulas is loading any more.
pub(super) fn is_stale(flow: &Flow, math: &mut Math, view: Rect) -> bool {
    if !Rect::from_min_size(Pos2::ZERO, flow.size).intersects(view) {
        return false;
    }
    // Neither loop may stop at its first hit. The formula cache drops a loading formula that
    // nobody asked for in a frame, and may drop a ready one, so every formula is asked for.
    if flow.is_waiting {
        let mut loading = false;
        for formula in flow.formulas.iter().filter(|f| f.seen != Seen::Failed) {
            loading |= math.get(&formula.math_ref()) == MathState::Loading;
        }
        return !loading;
    }
    let mut stale = false;
    for placed in flow.maths.iter().filter(|p| flow.is_seen(p.row, view)) {
        let formula = &flow.formulas[placed.formula];
        stale |= Seen::from(math.get(&formula.math_ref())) != formula.seen;
    }
    stale
}

pub(super) fn draw(painter: &Painter, flow: &Flow, origin: Pos2, math: &mut Math) {
    let shift = origin.to_vec2();
    let view = painter.clip_rect().translate(-shift);
    for fill in flow.code_fills.iter().filter(|f| flow.is_seen(f.row, view)) {
        painter.rect_filled(fill.rect.translate(shift), fill.radius, fill.color);
    }
    for run in flow.runs.iter().filter(|r| flow.is_seen(r.row, view)) {
        // egui keeps the laid-out text of a job from one frame to the next, so this is a lookup
        // while the run stays on screen.
        let galley = painter.layout_job(run.job.clone());
        let Some(glyph) = galley.rows.first().and_then(|row| row.row.glyphs.first()) else {
            continue;
        };
        // The `y` of a glyph is the baseline of its row.
        let top = flow.rows[run.row].baseline - run.raise - glyph.pos.y;
        painter.galley(origin + vec2(run.x, top), galley, Color32::PLACEHOLDER);
    }
    for placed in flow.maths.iter().filter(|p| flow.is_seen(p.row, view)) {
        let formula = &flow.formulas[placed.formula];
        if let MathState::Ready(image) = math.get(&formula.math_ref()) {
            let left = origin + vec2(placed.x, flow.rows[placed.row].baseline);
            image.paint(painter, left, placed.scale, placed.tint);
        }
    }
}

pub(super) fn block(
    ui: &mut Ui,
    flow: &Flow,
    text: &RichText<'_>,
    math: &mut Math,
) -> Option<Clicked> {
    let (rect, response) = ui.allocate_exact_size(flow.size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &flow.plain));
    if ui.is_rect_visible(rect) {
        draw(ui.painter(), flow, rect.min, math);
    }
    // The chips are added after the block, so a click on a chip goes to the chip.
    let mut clicked = None;
    for (index, chip) in flow.chips.iter().enumerate() {
        let at = chip.rect.translate(rect.min.to_vec2());
        if !ui.is_rect_visible(at) {
            continue;
        }
        let is_selected = text.selected == Some(chip.number);
        let id = response.id.with(("cite", index));
        if CitationChip::new(chip.number)
            .selected(is_selected)
            .show_at(ui, at, id)
            .clicked()
        {
            clicked = Some(Clicked::Citation(chip.number));
        }
    }
    if copy_is_chosen(&response, "Copy text") {
        clicked = Some(Clicked::CopyText(text.markdown.to_owned()));
    }
    clicked
}

/// Shows the menu that a secondary click opens, with one entry. True in the frame that the
/// entry is chosen.
pub(super) fn copy_is_chosen(response: &Response, label: &str) -> bool {
    let mut chosen = false;
    response.context_menu(|ui| {
        if ui.add(Button::ghost(label)).clicked() {
            chosen = true;
            ui.close();
        }
    });
    chosen
}
