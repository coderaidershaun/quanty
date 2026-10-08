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
