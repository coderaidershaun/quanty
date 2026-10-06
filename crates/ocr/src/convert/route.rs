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

#[cfg(test)]
mod tests {
    use super::*;

    type SetTags = fn(&mut PageCategories);

    fn tagged(set: SetTags) -> PageCategories {
        let mut tags = PageCategories::default();
        set(&mut tags);
        tags
    }

    #[test]
    fn route_sends_math_figures_tables_and_doubt_to_sonnet() {
        let none = PageCategories::default();

        // The one case that may go to Haiku. These four tags only describe the page's furniture:
        // its headings and numbers.
        let furniture = tagged(|tags| {
            tags.chapter_number = true;
            tags.chapter_name = true;
            tags.page_number = true;
            tags.sub_heading = true;
        });
        assert_eq!(route_reasons(&furniture, MathCheck::None, 300), []);

        let by_tag: [(&str, SetTags, RouteReason); 10] = [
            (
                "displayed math",
                |tags| tags.math_notation = true,
                RouteReason::TagsReportMath,
            ),
            (
                "inline math",
                |tags| tags.inline_with_text_math_notation = true,
                RouteReason::TagsReportMath,
            ),
            (
                "multi-axis chart",
                |tags| tags.diagram_2d_multi_axis_chart = true,
                RouteReason::TagsReportFigure,
            ),
            (
                "single-axis chart",
                |tags| tags.diagram_2d_single_axis_chart = true,
                RouteReason::TagsReportFigure,
            ),
            (
                "surface chart",
                |tags| tags.diagram_3d_surface_chart = true,
                RouteReason::TagsReportFigure,
            ),
            (
                "bar chart",
                |tags| tags.diagram_2d_bar_chart = true,
                RouteReason::TagsReportFigure,
            ),
            (
                "mixed chart",
                |tags| tags.diagram_2d_mixed_chart = true,
                RouteReason::TagsReportFigure,
            ),
            (
                "other diagram",
                |tags| tags.diagram_other = true,
                RouteReason::TagsReportFigure,
            ),
            (
                "image",
                |tags| tags.image = true,
                RouteReason::TagsReportFigure,
            ),
            (
                "table",
                |tags| tags.table = true,
                RouteReason::TagsReportTable,
            ),
        ];
        for (name, set, reason) in by_tag {
            assert_eq!(
                route_reasons(&tagged(set), MathCheck::None, 300),
                [reason],
                "{name}"
            );
        }

        let by_math_check = [
            (MathCheck::Inline, RouteReason::MathCheckReportsMath),
            (MathCheck::Block, RouteReason::MathCheckReportsMath),
            (MathCheck::Both, RouteReason::MathCheckReportsMath),
            (MathCheck::NoAnswer, RouteReason::MathCheckFailed),
        ];
        for (math_check, reason) in by_math_check {
            assert_eq!(
                route_reasons(&none, math_check, 300),
                [reason],
                "{math_check:?}"
            );
        }

        assert_eq!(
            route_reasons(&none, MathCheck::None, ALMOST_EMPTY_WORDS - 1),
            [RouteReason::TextLayerAlmostEmpty]
        );

        // Reasons come in the order they are found.
        let math_and_table = tagged(|tags| {
            tags.math_notation = true;
            tags.table = true;
        });
        assert_eq!(
            route_reasons(&math_and_table, MathCheck::Block, 5),
            [
                RouteReason::TagsReportMath,
                RouteReason::TagsReportTable,
                RouteReason::MathCheckReportsMath,
                RouteReason::TextLayerAlmostEmpty,
            ]
        );
    }
}
