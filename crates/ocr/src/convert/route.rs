//! The reasons a page goes to Sonnet that are known before any text is copied. The reasons a copy
//! is refused are worked out later, with the page itself.

use super::checks::ALMOST_EMPTY_WORDS;
use crate::content::{MathCheck, PageCategories, RouteReason};

/// An empty list means Haiku copies the page.
// SMELL: the reasons a page goes to Sonnet are decided in two places: here, before a copy is
// made, and where the copy is checked. A reader has to find both to see every reason.
pub(super) fn route_reasons(
    tags: &PageCategories,
    math_check: MathCheck,
    text_layer_words: usize,
) -> Vec<RouteReason> {
    let reports_figure = tags.diagram_2d_multi_axis_chart
        || tags.diagram_2d_single_axis_chart
        || tags.diagram_3d_surface_chart
        || tags.diagram_2d_bar_chart
        || tags.diagram_2d_mixed_chart
        || tags.diagram_other
        || tags.image;
    [
        (
            tags.math_notation || tags.inline_with_text_math_notation,
            RouteReason::TagsReportMath,
        ),
        (reports_figure, RouteReason::TagsReportFigure),
        (tags.table, RouteReason::TagsReportTable),
        (
            matches!(
                math_check,
                MathCheck::Inline | MathCheck::Block | MathCheck::Both
            ),
            RouteReason::MathCheckReportsMath,
        ),
        (
            math_check == MathCheck::NoAnswer,
            RouteReason::MathCheckFailed,
        ),
        (
            text_layer_words < ALMOST_EMPTY_WORDS,
            RouteReason::TextLayerAlmostEmpty,
        ),
    ]
    .into_iter()
    .filter_map(|(applies, reason)| applies.then_some(reason))
    .collect()
}
