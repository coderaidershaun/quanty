//! Every reason a page goes to Sonnet: the ones known before any text is copied, and the ones
//! found in a Haiku copy.

use super::checks::{ALMOST_EMPTY_WORDS, any_string_has_backslash, clean_and_check, word_match};
use super::reply::{CopiedPage, TranscribedPage};
use crate::content::{MathCheck, PageCategories, RouteReason};

// Both match ratios of a Haiku copy must reach this. A clean copy of the sample text page scored
// 0.995 and 1.0, and a false alarm only costs one Sonnet call.
const COPY_MATCH_MINIMUM: f64 = 0.96;

/// An empty list means Haiku copies the page.
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

pub(super) enum CopyOutcome {
    Kept(TranscribedPage),
    Refused(RouteReason),
}

pub(super) fn judge_copy(
    copy: CopiedPage,
    text_layer: &str,
    text_layer_words: usize,
) -> CopyOutcome {
    if copy.needs_stronger_model {
        return CopyOutcome::Refused(RouteReason::CopyFlaggedStrongerModel);
    }
    let mut page = TranscribedPage::from(copy);
    let checked = clean_and_check(&mut page, text_layer_words);
    let problem = if page.pieces.is_empty() {
        Some(RouteReason::CopyHasNoPieces)
    } else if any_string_has_backslash(&page) {
        Some(RouteReason::CopyContainsBackslash)
    } else if checked.is_err() {
        Some(RouteReason::CopyFailedReplyCheck)
    } else {
        let matched = word_match(&page, text_layer);
        (matched.piece_words_in_text_layer < COPY_MATCH_MINIMUM
            || matched.text_layer_words_in_pieces < COPY_MATCH_MINIMUM)
            .then_some(RouteReason::CopyFailedCopyCheck)
    };
    match problem {
        Some(reason) => CopyOutcome::Refused(reason),
        None => CopyOutcome::Kept(page),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CopyOutcome, judge_copy};
    use crate::content::RouteReason;
    use crate::convert::reply::{CopiedPage, TranscribedPiece};

    const TEXT_LAYER: &str = "let a = \"x\\ty\"; if a { b % c } $";

    fn a_copy_with_code(code: &str) -> CopiedPage {
        serde_json::from_value(json!({
            "needs-stronger-model": false,
            "printed-page-number": null,
            "running-header": null,
            "pieces": [{
                "kind": "code",
                "number": 1,
                "label": null,
                "caption": null,
                "language": " Rust ",
                "code": code,
                "summary": "Compares two values."
            }],
            "starts-mid-sentence": false,
            "ends-mid-sentence": false
        }))
        .unwrap()
    }

    #[test]
    fn a_haiku_copy_keeps_its_code_listing_and_refuses_one_with_a_tab() {
        let indented = "\n    let a = \"x\\ty\";   \n\n    if a { b % c } $\n  \n";

        let CopyOutcome::Kept(page) = judge_copy(a_copy_with_code(indented), TEXT_LAYER, 8) else {
            panic!("a copy that holds only code was refused");
        };
        let TranscribedPiece::Code { code, language, .. } = &page.pieces[0] else {
            panic!("the kept piece is not code");
        };
        assert_eq!(code, "    let a = \"x\\ty\";\n\n    if a { b % c } $");
        assert_eq!(language.as_deref(), Some("rust"));

        let tabbed = "\tlet a = \"x\\ty\"; if a { b % c } $";
        assert!(matches!(
            judge_copy(a_copy_with_code(tabbed), TEXT_LAYER, 8),
            CopyOutcome::Refused(RouteReason::CopyFailedReplyCheck)
        ));
    }
}
