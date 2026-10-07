//! Works out the rectangle to cut for a figure. The model's top and bottom edges cannot be trusted
//! where body text sits next to a figure, but the page's text lines say where the text really is.

use crate::content::PageBox;
use crate::convert::checks::words;
use crate::convert::poppler::TextLine;

/// Words in a row that two texts must share before a line counts as copied from one of them.
/// Single words and pairs recur between a figure and its paragraphs; four in a row do not.
const MIN_RUN: usize = 4;
/// How far outside the model's rectangle, in thousandths of the page, one of the figure's own
/// lines may be and still be taken in unchecked. Tick values and axis titles sit within a line or
/// two of the plot.
const GROW_REACH: i32 = 30;
/// How far a trimmed edge stays from the line it was trimmed at, in thousandths of the page.
/// Printed letters reach about 2 outside the boxes the text layer gives them, and the narrowest
/// gap measured between a figure and the text next to it is 20.
const TRIM_MARGIN: i32 = 6;

pub(super) struct FigureCut {
    /// Padding included. Always a usable rectangle.
    pub area: PageBox,
    /// A line of another piece of the page is still wholly inside `area`.
    pub holds_body_text: bool,
    /// The model's rectangle could not be checked: none of the figure's own lines was found at it,
    /// or the refined rectangle was unusable. `area` is then that rectangle with padding, as given.
    pub unchecked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// The figure. A tied line matches another piece's text as well as it matches the figure's.
    Figure {
        is_tied: bool,
    },
    Other,
    Neither,
}

struct Classified {
    area: PageBox,
    owner: Owner,
}

/// `bounds` is the rectangle the model gave, which must already be usable; its left and right
/// edges only ever move outward. `own` is every string the figure printed, and `other` every
/// string the rest of the page copied from the page.
// SMELL: `own` and `other` have the same type, so a call that swaps them compiles and gets every
// decision the wrong way round. One type holding both lists would make the swap impossible.
pub(super) fn cut_rectangle(
    bounds: PageBox,
    own: &[String],
    other: &[String],
    lines: &[TextLine],
) -> FigureCut {
    let own: Vec<Vec<String>> = own.iter().map(|text| words(text)).collect();
    let other: Vec<Vec<String>> = other.iter().map(|text| words(text)).collect();
    let lines: Vec<Classified> = lines
        .iter()
        .map(|line| Classified {
            area: line.area,
            owner: owner_of(&words(&line.text), &own, &other),
        })
        .collect();

    // Padding goes on before the trim: padding a trimmed rectangle would take the first line of
    // the next paragraph straight back in.
    let refined = trimmed(grown(bounds, &lines).padded(), &lines)
        .filter(|refined| refined.problem().is_none());
    let unchecked = refined.is_none();
    let area = refined.unwrap_or_else(|| bounds.padded());
    let holds_body_text = lines
        .iter()
        .any(|line| line.owner == Owner::Other && is_inside(line.area, area));
    FigureCut {
        area,
        holds_body_text,
        unchecked,
    }
}

fn owner_of(line: &[String], own: &[Vec<String>], other: &[Vec<String>]) -> Owner {
    let count = line.len();
    if count == 0 {
        return Owner::Neither;
    }
    let longest = |strings: &[Vec<String>]| {
        strings
            .iter()
            .map(|string| longest_run(line, string))
            .max()
            .unwrap_or(0)
    };
    let (own_run, other_run) = (longest(own), longest(other));
    let is_a_whole_other_string = count >= 2 && other.iter().any(|string| string == line);
    if other_run > own_run && (other_run >= MIN_RUN || is_a_whole_other_string) {
        return Owner::Other;
    }
    if own_run < other_run {
        return Owner::Neither;
    }
    let belongs_to_figure = if count == 1 {
        // A line of one word is the figure's only when no other piece has that word, and it is
        // never counted as another piece's: clipping a figure is worse than keeping a stray word.
        own.iter().any(|string| string.contains(&line[0]))
            && !other.iter().any(|string| string.contains(&line[0]))
    } else {
        own_run == count || own_run >= MIN_RUN
    };
    if belongs_to_figure {
        Owner::Figure {
            is_tied: own_run == other_run,
        }
    } else {
        Owner::Neither
    }
}

/// The most consecutive words of `line` that also appear consecutively in `string`.
fn longest_run(line: &[String], string: &[String]) -> usize {
    let mut best = 0;
    let mut previous = vec![0; string.len() + 1];
    for word in line {
        let mut current = vec![0; string.len() + 1];
        for (index, candidate) in string.iter().enumerate() {
            if word == candidate {
                current[index + 1] = previous[index] + 1;
                best = best.max(current[index + 1]);
            }
        }
        previous = current;
    }
    best
}

/// One of the figure's own lines beyond [`GROW_REACH`] is taken in only when it is above or
/// below `bounds` with no line of another piece in between, because the model sometimes leaves
/// out a label line far above a frame. Every line is measured against the model's rectangle, so
/// taking one line in never brings the next one into reach.
fn grown(bounds: PageBox, lines: &[Classified]) -> PageBox {
    let reach = PageBox {
        left: bounds.left - GROW_REACH,
        top: bounds.top - GROW_REACH,
        right: bounds.right + GROW_REACH,
        bottom: bounds.bottom + GROW_REACH,
    };
    let touching = figure_lines_touching(bounds, lines);
    let untied = untied_areas(&touching);
    // What stands for the figure when looking for a line in between: its own lines that touch
    // `bounds`, the untied ones when there are any. With none, `bounds` itself stands for it.
    let anchors: Vec<PageBox> = if untied.is_empty() {
        touching.iter().map(|(area, _)| *area).collect()
    } else {
        untied
    };
    let neighbours: Vec<PageBox> = lines
        .iter()
        .filter(|line| line.owner == Owner::Other && overlaps_sideways(line.area, bounds))
        .map(|line| line.area)
        .collect();
    let is_clear_of_neighbours = |line: PageBox| {
        let is_above_or_below = line.bottom < bounds.top || line.top > bounds.bottom;
        let figure = nearest(line, &anchors).unwrap_or(bounds);
        is_above_or_below
            && overlaps_sideways(line, bounds)
            && !neighbours
                .iter()
                .any(|between| lies_between(*between, line, figure))
    };
    let mut area = bounds;
    for line in lines.iter().filter(|line| is_figure_line(line)) {
        if touches(reach, line.area) || is_clear_of_neighbours(line.area) {
            area = PageBox {
                left: area.left.min(line.area.left),
                top: area.top.min(line.area.top),
                right: area.right.max(line.area.right),
                bottom: area.bottom.max(line.area.bottom),
            };
        }
    }
    area
}

fn figure_lines_touching(rectangle: PageBox, lines: &[Classified]) -> Vec<(PageBox, bool)> {
    lines
        .iter()
        .filter_map(|line| match line.owner {
            Owner::Figure { is_tied } if touches(rectangle, line.area) => {
                Some((line.area, is_tied))
            }
            _ => None,
        })
        .collect()
}

fn untied_areas(lines: &[(PageBox, bool)]) -> Vec<PageBox> {
    lines
        .iter()
        .filter(|(_, is_tied)| !is_tied)
        .map(|(area, _)| *area)
        .collect()
}

/// The top and bottom edges move in to [`TRIM_MARGIN`] from the lines of other pieces that sit
/// above or below the figure's own lines, and never in past the figure's own lines. `None` when
/// none of the figure's own lines is there to trim against.
fn trimmed(padded: PageBox, lines: &[Classified]) -> Option<PageBox> {
    let neighbours: Vec<PageBox> = lines
        .iter()
        .filter(|line| line.owner == Owner::Other && overlaps_sideways(line.area, padded))
        .map(|line| line.area)
        .collect();
    let (span_top, span_bottom) = figure_span(padded, lines, &neighbours)?;
    let mut area = padded;
    for neighbour in neighbours {
        if neighbour.bottom <= span_top {
            area.top = area.top.max(neighbour.bottom + TRIM_MARGIN);
        } else if neighbour.top >= span_bottom {
            area.bottom = area.bottom.min(neighbour.top - TRIM_MARGIN);
        }
    }
    area.top = area.top.min(span_top);
    area.bottom = area.bottom.max(span_bottom);
    Some(area)
}

/// From the top of the highest to the bottom of the lowest of the figure's own lines that touch
/// `padded` and count. A tied line counts only when no neighbour lies between it and the nearest
/// untied line, because table cells and the short last lines of paragraphs often read exactly
/// like a legend entry; with no untied line, tied lines count like any other.
fn figure_span(
    padded: PageBox,
    lines: &[Classified],
    neighbours: &[PageBox],
) -> Option<(i32, i32)> {
    let touching = figure_lines_touching(padded, lines);
    let untied = untied_areas(&touching);
    let counted = touching.iter().filter(|(area, is_tied)| {
        !is_tied
            || untied.is_empty()
            || nearest(*area, &untied).is_none_or(|anchor| {
                !neighbours
                    .iter()
                    .any(|between| lies_between(*between, *area, anchor))
            })
    });
    counted.fold(None, |span, (area, _)| match span {
        None => Some((area.top, area.bottom)),
        Some((top, bottom)) => Some((top.min(area.top), bottom.max(area.bottom))),
    })
}

/// Twice the vertical centre, so that no rounding is needed.
fn doubled_centre(area: PageBox) -> i32 {
    area.top + area.bottom
}

fn nearest(from: PageBox, candidates: &[PageBox]) -> Option<PageBox> {
    candidates
        .iter()
        .copied()
        .min_by_key(|candidate| (doubled_centre(*candidate) - doubled_centre(from)).abs())
}

fn lies_between(middle: PageBox, one: PageBox, other: PageBox) -> bool {
    let (centre, one, other) = (
        doubled_centre(middle),
        doubled_centre(one),
        doubled_centre(other),
    );
    one.min(other) < centre && centre < one.max(other)
}

fn is_figure_line(line: &Classified) -> bool {
    matches!(line.owner, Owner::Figure { .. })
}

fn touches(one: PageBox, other: PageBox) -> bool {
    one.left <= other.right
        && other.left <= one.right
        && one.top <= other.bottom
        && other.top <= one.bottom
}

/// True when the two overlap sideways across at least half of the narrower of the two.
fn overlaps_sideways(line: PageBox, rectangle: PageBox) -> bool {
    let overlap = line.right.min(rectangle.right) - line.left.max(rectangle.left);
    let narrower = (line.right - line.left).min(rectangle.right - rectangle.left);
    overlap > 0 && overlap * 2 >= narrower
}

fn is_inside(line: PageBox, rectangle: PageBox) -> bool {
    line.left >= rectangle.left
        && line.right <= rectangle.right
        && line.top >= rectangle.top
        && line.bottom <= rectangle.bottom
}
