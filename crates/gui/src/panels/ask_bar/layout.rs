//! Where each control of the bar sits. The bar picks one of three arrangements by its own
//! width, and in each one every control has a fixed slot, so nothing moves when a media title is
//! long.

use eframe::egui;

use super::note::NoteRow;
use crate::theme::{size, space, stroke};
use crate::widgets::{Button, ControlSize};

/// The sides are as wide as the outline and margin of a sheet, so the controls line up with the
/// content of the sheets below.
const SIDE: f32 = space::LG + stroke::BORDER;
pub(crate) const TWO_ROWS: f32 = size::CONTROL_LG + space::SM + size::CONTROL_SM;
/// The question and Ask over the mode, the depth and the one button for all the filters.
const NARROW_ROWS: f32 = size::CONTROL_LG + space::SM + size::CONTROL_MD;
/// The note stands on a row of its own under the rows of a narrow bar, as tall as the button that
/// may stand in it for a finger.
const NOTE_ROW: f32 = space::XS + size::CONTROL_MD;
const ASK_WIDTH: f32 = Button::least_width(ControlSize::Large);
pub(super) const PICKER_WIDTH: f32 = 4.5 * size::CONTROL_LG;
/// The category list holds short words, so its slot is narrower and leaves the note more room.
const CATEGORY_WIDTH: f32 = 3.5 * size::CONTROL_LG;
/// The button for the filters that have no slot is as wide as the narrowest slot.
const FILTERS_WIDTH: f32 = CATEGORY_WIDTH;
/// On a narrow bar the button for the filters is as small as its words allow, so that the mode and
/// the depth beside it can be read.
const NARROW_FILTERS_WIDTH: f32 = 2.5 * size::CONTROL_LG;
/// The share of the room beside the filters button that the mode gets on a narrow bar, because
/// "Results only" is the longest word of the two.
const NARROW_MODE_SHARE: f32 = 0.55;
/// The note keeps at least this much of its row, so a reason not to ask can be read.
const NOTE_LEAST: f32 = 200.0;
/// A question box narrower than this shows too few words, so the mode and the depth move down to
/// the second row.
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
    pub(super) depth: egui::Rect,
    pub(super) mode_size: ControlSize,
    pub(super) ask: egui::Rect,
    /// The slot of each filter that stands on the bar, in the order of the filters. The filters
    /// after the last one with a slot are in the sheet that `more` opens.
    pub(super) filters: [Option<egui::Rect>; FILTER_COUNT],
    /// The button for the filters with no slot. `None` when all five stand on the bar.
    pub(super) more: Option<egui::Rect>,
    pub(super) more_size: ControlSize,
    /// `None` on a narrow bar that has no row for a note.
    pub(super) note: Option<egui::Rect>,
    /// The size of the button that a note may hold.
    pub(super) note_size: ControlSize,
}

/// Whether a bar that wide is narrow, so that it stands in three rows.
fn is_narrow(bar_width: f32) -> bool {
    let width = bar_width - 2.0 * SIDE;
    width
        < ASK_WIDTH
            + space::SM
            + PICKER_WIDTH
            + space::SM
            + CATEGORY_WIDTH
            + space::SM
            + QUESTION_LEAST
}

/// The height of the rows that `arrange` fills in a bar that wide, without the margin around them.
/// The note stands beside the filters of a bar with two rows, so only a narrow bar gives it a row
/// of its own.
pub(crate) fn rows_height(bar_width: f32, note: NoteRow) -> f32 {
    match (is_narrow(bar_width), note) {
        (false, _) => TWO_ROWS,
        (true, NoteRow::Absent) => NARROW_ROWS,
        (true, NoteRow::Shown) => NARROW_ROWS + NOTE_ROW,
    }
}

/// Places the controls in `bar`, the whole rectangle the window gives the bar.
pub(super) fn arrange(bar: egui::Rect, note: NoteRow) -> Rects {
    let height = rows_height(bar.width(), note);
    let top = bar.top() + ((bar.height() - height) / 2.0).round();
    let content = egui::Rect::from_min_size(
        egui::pos2(bar.left() + SIDE, top),
        egui::vec2(bar.width() - 2.0 * SIDE, height),
    );
    if is_narrow(bar.width()) {
        three_rows(content, note)
    } else {
        two_rows(content)
    }
}

/// The question, the depth, the mode and Ask, over as many filters as fit and the note.
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
    let depth = egui::Rect::from_min_max(
        egui::pos2(mode.left() - space::SM - CATEGORY_WIDTH, first.top()),
        egui::pos2(mode.left() - space::SM, first.bottom()),
    );
    let question = egui::Rect::from_min_max(
        first.min,
        egui::pos2(depth.left() - space::SM, first.bottom()),
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
        depth,
        mode_size: ControlSize::Large,
        ask,
        filters,
        more,
        more_size: ControlSize::Small,
        note: Some(note),
        note_size: ControlSize::Small,
    }
}

/// The question and Ask over the mode, the depth and the one button for all the filters, over the
/// note when there is one.
fn three_rows(content: egui::Rect, note: NoteRow) -> Rects {
    let first =
        egui::Rect::from_min_size(content.min, egui::vec2(content.width(), size::CONTROL_LG));
    let second = egui::Rect::from_min_size(
        egui::pos2(content.left(), first.bottom() + space::SM),
        egui::vec2(content.width(), size::CONTROL_MD),
    );
    let third = egui::Rect::from_min_size(
        egui::pos2(content.left(), second.bottom() + space::XS),
        egui::vec2(content.width(), size::CONTROL_MD),
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
        egui::pos2(second.right() - NARROW_FILTERS_WIDTH, second.top()),
        second.max,
    );
    let room = more.left() - space::SM - second.left() - space::SM;
    let mode_width = PICKER_WIDTH.min(room * NARROW_MODE_SHARE);
    let depth_width = CATEGORY_WIDTH.min(room - mode_width);
    let mode = egui::Rect::from_min_size(second.min, egui::vec2(mode_width, second.height()));
    let depth = egui::Rect::from_min_size(
        egui::pos2(mode.right() + space::SM, second.top()),
        egui::vec2(depth_width, second.height()),
    );
    Rects {
        question,
        mode,
        depth,
        mode_size: ControlSize::Medium,
        ask,
        filters: [None; FILTER_COUNT],
        more: Some(more),
        more_size: ControlSize::Medium,
        note: (note == NoteRow::Shown).then_some(third),
        note_size: ControlSize::Medium,
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

    fn slots(rects: &Rects) -> Vec<(String, egui::Rect)> {
        let mut slots = vec![
            ("question".to_owned(), rects.question),
            ("mode".to_owned(), rects.mode),
            ("depth".to_owned(), rects.depth),
            ("ask".to_owned(), rects.ask),
        ];
        if let Some(note) = rects.note {
            slots.push(("note".to_owned(), note));
        }
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
            for note in [NoteRow::Absent, NoteRow::Shown] {
                let rows = rows_height(width as f32, note);
                // A short window of any width is compact. A window at least 720 points wide can
                // also be medium or wide, and then its bar is at least 688 points wide.
                let heights: &[f32] = if width >= 688 {
                    &[2.0 * space::SM + rows, 2.0 * space::MD + rows]
                } else {
                    &[2.0 * space::SM + rows]
                };
                for &height in heights {
                    let bar = egui::Rect::from_min_size(
                        egui::pos2(16.0, 52.0),
                        egui::vec2(width as f32, height),
                    );
                    let rects = arrange(bar, note);
                    let inside = egui::Rect::from_min_max(
                        egui::pos2(bar.left() + SIDE, bar.top()),
                        egui::pos2(bar.right() - SIDE, bar.bottom()),
                    );
                    let slots = slots(&rects);
                    for (name, rect) in &slots {
                        assert!(
                            inside.contains_rect(*rect)
                                && rect.width() > 0.0
                                && rect.height() > 0.0,
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
                    if width < 674 {
                        assert!(
                            rects.mode.top() > rects.question.bottom()
                                && rects.depth.top() > rects.question.bottom(),
                            "at {width} the mode and the depth stand under the question"
                        );
                    }
                    assert_eq!(
                        rects.note.is_some(),
                        width >= 674 || note == NoteRow::Shown,
                        "at {width} the note has a slot beside the filters or, on a narrow bar, \
                         only while a note shows"
                    );
                }
            }
        }
    }
}
