//! Where each control of the bar sits. The bar picks one of three arrangements by its own
//! width, and in each one every control has a fixed slot, so nothing moves when a media title is
//! long.

use eframe::egui;

use crate::theme::{size, space, stroke};
use crate::widgets::{Button, ControlSize};

/// The sides are as wide as the outline and margin of a sheet, so the controls line up with the
/// content of the sheets below.
const SIDE: f32 = space::LG + stroke::BORDER;
const TWO_ROWS: f32 = size::CONTROL_LG + space::SM + size::CONTROL_SM;
// SMELL: the window's layout gives the bar of a compact window this height and a small margin
// above and below, in a sum of its own. A change here must be made there too, or the rows leave
// the bar on a phone.
const THREE_ROWS: f32 =
    size::CONTROL_LG + space::SM + size::CONTROL_MD + space::XS + size::CONTROL_SM;
const ASK_WIDTH: f32 = Button::least_width(ControlSize::Large);
pub(super) const PICKER_WIDTH: f32 = 4.5 * size::CONTROL_LG;
/// The category list holds short words, so its slot is narrower and leaves the note more room.
const CATEGORY_WIDTH: f32 = 3.5 * size::CONTROL_LG;
/// The button for the filters that have no slot is as wide as the narrowest slot.
const FILTERS_WIDTH: f32 = CATEGORY_WIDTH;
/// The note keeps at least this much of its row, so a reason not to ask can be read.
const NOTE_LEAST: f32 = 200.0;
/// A question box narrower than this shows too few words, so the mode moves down to the second
/// row.
const QUESTION_LEAST: f32 = 200.0;

pub(super) const FILTER_COUNT: usize = 5;
/// The width of the slot of each filter, in the order the filters stand on the bar.
// SMELL: the filters are listed in this order a second time, where they are drawn. A change of
// order there must be made here too, or a filter gets the slot of another.
const SLOT_WIDTHS: [f32; FILTER_COUNT] = [
    PICKER_WIDTH,
    PICKER_WIDTH,
    PICKER_WIDTH,
    PICKER_WIDTH,
    CATEGORY_WIDTH,
];

#[derive(Debug, Clone, Copy)]
pub(super) struct Rects {
    pub(super) question: egui::Rect,
    pub(super) mode: egui::Rect,
    pub(super) mode_size: ControlSize,
    pub(super) ask: egui::Rect,
    /// The slot of each filter that stands on the bar, in the order of the filters. The filters
    /// after the last one with a slot are in the sheet that `more` opens.
    pub(super) filters: [Option<egui::Rect>; FILTER_COUNT],
    /// The button for the filters with no slot. `None` when all five stand on the bar.
    pub(super) more: Option<egui::Rect>,
    pub(super) more_size: ControlSize,
    pub(super) note: egui::Rect,
}

/// Places the controls in `bar`, the whole rectangle the window gives the bar.
pub(super) fn arrange(bar: egui::Rect) -> Rects {
    let width = bar.width() - 2.0 * SIDE;
    let is_narrow = width < ASK_WIDTH + space::SM + PICKER_WIDTH + space::SM + QUESTION_LEAST;
    let height = if is_narrow { THREE_ROWS } else { TWO_ROWS };
    let top = bar.top() + ((bar.height() - height) / 2.0).round();
    let content = egui::Rect::from_min_size(
        egui::pos2(bar.left() + SIDE, top),
        egui::vec2(width, height),
    );
    if is_narrow {
        three_rows(content)
    } else {
        two_rows(content)
    }
}

/// The question, the mode and Ask, over as many filters as fit and the note.
fn two_rows(content: egui::Rect) -> Rects {
    let first =
        egui::Rect::from_min_size(content.min, egui::vec2(content.width(), size::CONTROL_LG));
    let second = egui::Rect::from_min_size(
        egui::pos2(content.left(), first.bottom() + space::SM),
        egui::vec2(content.width(), size::CONTROL_SM),
    );
    let ask = egui::Rect::from_min_max(
        egui::pos2(first.right() - ASK_WIDTH, first.top()),
        first.max,
    );
    let mode = egui::Rect::from_min_max(
        egui::pos2(ask.left() - space::SM - PICKER_WIDTH, first.top()),
        egui::pos2(ask.left() - space::SM, first.bottom()),
    );
    let question = egui::Rect::from_min_max(
        first.min,
        egui::pos2(mode.left() - space::SM, first.bottom()),
    );

    let inline = inline_filters(second.width());
    let mut filters = [None; FILTER_COUNT];
    let mut next = second.left();
    for (place, width) in SLOT_WIDTHS.into_iter().enumerate().take(inline) {
        let slot = egui::Rect::from_min_size(
            egui::pos2(next, second.top()),
            egui::vec2(width, size::CONTROL_SM),
        );
        filters[place] = Some(slot);
        next = slot.right() + space::SM;
    }
    let more = (inline < FILTER_COUNT).then(|| {
        egui::Rect::from_min_size(
            egui::pos2(next, second.top()),
            egui::vec2(FILTERS_WIDTH, size::CONTROL_SM),
        )
    });
    let filters_end = more
        .into_iter()
        .chain(filters.into_iter().flatten())
        .map(|slot| slot.right())
        .fold(second.left(), f32::max);
    let note = egui::Rect::from_min_max(
        egui::pos2(filters_end + space::MD, second.top()),
        second.max,
    );
    Rects {
        question,
        mode,
        mode_size: ControlSize::Large,
        ask,
        filters,
        more,
        more_size: ControlSize::Small,
        note,
    }
}

/// The question and Ask over the mode and the one button for all the filters, over the note.
fn three_rows(content: egui::Rect) -> Rects {
    let first =
        egui::Rect::from_min_size(content.min, egui::vec2(content.width(), size::CONTROL_LG));
    let second = egui::Rect::from_min_size(
        egui::pos2(content.left(), first.bottom() + space::SM),
        egui::vec2(content.width(), size::CONTROL_MD),
    );
    let third = egui::Rect::from_min_size(
        egui::pos2(content.left(), second.bottom() + space::XS),
        egui::vec2(content.width(), size::CONTROL_SM),
    );
    let ask = egui::Rect::from_min_max(
        egui::pos2(first.right() - ASK_WIDTH, first.top()),
        first.max,
    );
    let question = egui::Rect::from_min_max(
        first.min,
        egui::pos2(ask.left() - space::SM, first.bottom()),
    );
    let more = egui::Rect::from_min_max(
        egui::pos2(second.right() - FILTERS_WIDTH, second.top()),
        second.max,
    );
    let mode_width = PICKER_WIDTH.min(more.left() - space::SM - second.left());
    let mode = egui::Rect::from_min_size(second.min, egui::vec2(mode_width, second.height()));
    Rects {
        question,
        mode,
        mode_size: ControlSize::Medium,
        ask,
        filters: [None; FILTER_COUNT],
        more: Some(more),
        more_size: ControlSize::Medium,
        note: third,
    }
}

/// How many filters stand on a second row `width` wide: as many as leave room for the button for
/// the rest and for the least the note may have.
fn inline_filters(width: f32) -> usize {
    (0..=FILTER_COUNT)
        .rev()
        .find(|&inline| width >= second_row_needs(inline))
        .unwrap_or(0)
}

fn second_row_needs(inline: usize) -> f32 {
    let has_more = inline < FILTER_COUNT;
    let controls = inline + usize::from(has_more);
    let slots: f32 = SLOT_WIDTHS[..inline].iter().sum();
    let more = if has_more { FILTERS_WIDTH } else { 0.0 };
    slots + more + space::SM * controls.saturating_sub(1) as f32 + space::MD + NOTE_LEAST
}

/// The slot is not clipped, because the kit paints focus rings outside a control.
pub(super) fn slot<R>(
    ui: &mut egui::Ui,
    name: &str,
    rect: egui::Rect,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let builder = egui::UiBuilder::new()
        .id_salt(name)
        .max_rect(rect)
        .layout(egui::Layout::left_to_right(egui::Align::Center));
    ui.scope_builder(builder, add).inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::layout::{ASK_BAR, ASK_BAR_COMPACT, ASK_BAR_MEDIUM};

    fn slots(rects: &Rects) -> Vec<(String, egui::Rect)> {
        let mut slots = vec![
            ("question".to_owned(), rects.question),
            ("mode".to_owned(), rects.mode),
            ("ask".to_owned(), rects.ask),
            ("note".to_owned(), rects.note),
        ];
        for (place, rect) in rects.filters.iter().enumerate() {
            if let Some(rect) = rect {
                slots.push((format!("filter {place}"), *rect));
            }
        }
        if let Some(more) = rects.more {
            slots.push(("more".to_owned(), more));
        }
        slots
    }

    fn overlap(a: egui::Rect, b: egui::Rect) -> bool {
        let both = a.intersect(b);
        both.width() > 0.0 && both.height() > 0.0
    }

    #[test]
    fn every_slot_of_the_bar_lies_in_it_at_every_width() {
        for width in 344..=2528 {
            // A short window of any width is compact. A window at least 720 points wide can also
            // be medium or wide, and then its bar is at least 688 points wide.
            let heights: &[f32] = if width >= 688 {
                &[ASK_BAR_COMPACT, ASK_BAR_MEDIUM, ASK_BAR]
            } else {
                &[ASK_BAR_COMPACT]
            };
            for &height in heights {
                let bar = egui::Rect::from_min_size(
                    egui::pos2(16.0, 52.0),
                    egui::vec2(width as f32, height),
                );
                let rects = arrange(bar);
                let inside = egui::Rect::from_min_max(
                    egui::pos2(bar.left() + SIDE, bar.top()),
                    egui::pos2(bar.right() - SIDE, bar.bottom()),
                );
                let slots = slots(&rects);
                for (name, rect) in &slots {
                    assert!(
                        inside.contains_rect(*rect) && rect.width() > 0.0 && rect.height() > 0.0,
                        "at {width} x {height} the {name} slot {rect:?} is not inside {inside:?}"
                    );
                }
                for (at, (first, a)) in slots.iter().enumerate() {
                    for (second, b) in &slots[at + 1..] {
                        assert!(
                            !overlap(*a, *b),
                            "at {width} x {height} the {first} slot {a:?} covers the {second} slot {b:?}"
                        );
                    }
                }

                let inline = rects.filters.iter().flatten().count();
                assert!(
                    rects.filters[..inline].iter().all(Option::is_some),
                    "at {width} the filters on the bar are the first ones, in order"
                );
                assert_eq!(
                    rects.more.is_some(),
                    inline < FILTER_COUNT,
                    "at {width} the button for the rest stands exactly when a filter has no slot"
                );
                if width >= 1148 {
                    assert_eq!(
                        inline, FILTER_COUNT,
                        "a wide window (bar of 1148 or more) keeps all five filters on the bar"
                    );
                }
                if width < 526 {
                    assert!(
                        rects.mode.top() > rects.question.bottom(),
                        "at {width} the mode stands under the question"
                    );
                }
            }
        }
    }
}
