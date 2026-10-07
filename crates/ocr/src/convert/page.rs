//! One page from start to finish: tag it, choose Haiku or Sonnet, check what comes back, cut out
//! its figures and save it.

use std::path::{Path, PathBuf};

use super::checks::{
    ReplyFault, any_string_has_backslash, check_reply, clean_reply, piece_word_count,
    text_layer_word_count, word_match,
};
use super::figure::cut_figures;
use super::reply::{TranscribedPage, TranscribedPiece};
use super::route::route_reasons;
use super::save::write_page;
use super::services::{CallUsage, MathPlacement, PageServices, PageSource, ServiceError};
use super::summary::CallTally;
use crate::content::{
    CallRecord, CallStep, Checks, ContentError, Conversion, MathCheck, PageCategories,
    REJECTED_REPLY_FILE, Route, RouteReason, page_folder_name, partial_page_folder_name,
};

// Both match ratios of a Haiku copy must reach this. A clean copy of the sample text page scored
// 0.995 and 1.0, and a false alarm only costs one Sonnet call.
const COPY_MATCH_MINIMUM: f64 = 0.96;

#[derive(thiserror::Error, Debug)]
pub enum PageError {
    #[error(transparent)]
    Service(#[from] ServiceError),

    #[error(
        "the reply for the page was rejected twice; the last reply is saved at {}",
        saved_reply.display()
    )]
    ReplyRejected {
        /// What was wrong with the second reply.
        #[source]
        fault: ReplyFault,
        saved_reply: PathBuf,
    },

    #[error(transparent)]
    Content(#[from] ContentError),
}

#[derive(Default)]
struct Ledger {
    records: Vec<CallRecord>,
    tally: CallTally,
}

impl Ledger {
    fn add(&mut self, step: CallStep, usage: &CallUsage) {
        match step {
            CallStep::Tag => self.tally.tag += 1,
            CallStep::Copy => self.tally.copy += 1,
            CallStep::Transcribe => self.tally.transcribe += 1,
        }
        self.tally.cost_usd += usage.cost_usd;
        self.records.push(call_record(step, usage));
    }
}

/// What a paid call leaves in the saved files.
pub(super) fn call_record(step: CallStep, usage: &CallUsage) -> CallRecord {
    CallRecord {
        step,
        model: usage.model.clone(),
        cost_usd: usage.cost_usd,
        output_tokens: usage.output_tokens,
        thinking_tokens: usage.thinking_tokens,
        seconds: usage.seconds,
    }
}

fn math_check_of(answer: Result<Option<MathPlacement>, ServiceError>) -> MathCheck {
    match answer {
        Ok(Some(MathPlacement::Inline)) => MathCheck::Inline,
        Ok(Some(MathPlacement::Block)) => MathCheck::Block,
        Ok(Some(MathPlacement::Both)) => MathCheck::Both,
        Ok(None) => MathCheck::None,
        // SMELL: why the math check failed is dropped here. Only `no-answer` is saved, so a run
        // where every page goes to Sonnet for this reason does not say what went wrong.
        Err(_) => MathCheck::NoAnswer,
    }
}

enum CopyOutcome {
    Kept(TranscribedPage),
    Refused(RouteReason),
}

struct Written {
    page: TranscribedPage,
    route: Route,
    retry_reason: Option<String>,
}

struct PageRun<'a, S> {
    services: &'a S,
    source: &'a PageSource,
    partial_folder: &'a Path,
    text_layer_words: usize,
    ledger: Ledger,
}

impl<S: PageServices> PageRun<'_, S> {
    /// The tags are needed to save the page, so a failed tagging fails it. A failed math check
    /// does not: the page goes to Sonnet instead.
    async fn tag_and_check_math(&mut self) -> Result<(PageCategories, MathCheck), PageError> {
        let (tagged, math) = tokio::join!(
            self.services.tag(self.source),
            self.services.contains_math(self.source)
        );
        let tags = tagged?;
        self.ledger.add(CallStep::Tag, &tags.usage);
        self.ledger.tally.math_check += 1;
        Ok((tags.value, math_check_of(math)))
    }

    /// Haiku copies a page that has no reason to go to Sonnet. Every other page goes to Sonnet,
    /// and so does a page whose copy cannot be kept: that reason is then added to `reasons`.
    async fn write_by_route(
        &mut self,
        reasons: &mut Vec<RouteReason>,
    ) -> Result<Written, PageError> {
        let mut route = Route::Sonnet;
        if reasons.is_empty() {
            match self.copy_checked().await? {
                CopyOutcome::Kept(page) => {
                    return Ok(Written {
                        page,
                        route: Route::HaikuCopy,
                        retry_reason: None,
                    });
                }
                CopyOutcome::Refused(reason) => {
                    reasons.push(reason);
                    route = Route::HaikuThenSonnet;
                }
            }
        }
        let (page, retry_reason) = self.transcribe_checked().await?;
        Ok(Written {
            page,
            route,
            retry_reason,
        })
    }

    /// Asks Haiku for a plain copy and keeps it only when nothing about it is in doubt.
    async fn copy_checked(&mut self) -> Result<CopyOutcome, PageError> {
        let copy = self.services.copy(self.source).await?;
        self.ledger.add(CallStep::Copy, &copy.usage);
        let asked_for_stronger_model = copy.value.needs_stronger_model;
        let mut page = TranscribedPage::from(copy.value);
        clean_reply(&mut page);
        if asked_for_stronger_model {
            return Ok(CopyOutcome::Refused(RouteReason::CopyFlaggedStrongerModel));
        }
        Ok(match self.copy_problem(&page) {
            Some(reason) => CopyOutcome::Refused(reason),
            None => CopyOutcome::Kept(page),
        })
    }

    fn copy_problem(&self, copy: &TranscribedPage) -> Option<RouteReason> {
        if copy.pieces.is_empty() {
            return Some(RouteReason::CopyHasNoPieces);
        }
        if any_string_has_backslash(copy) {
            return Some(RouteReason::CopyContainsBackslash);
        }
        if check_reply(copy, self.text_layer_words).is_err() {
            return Some(RouteReason::CopyFailedReplyCheck);
        }
        let matched = word_match(copy, &self.source.text_layer);
        (matched.piece_words_in_text_layer < COPY_MATCH_MINIMUM
            || matched.text_layer_words_in_pieces < COPY_MATCH_MINIMUM)
            .then_some(RouteReason::CopyFailedCopyCheck)
    }

    /// Asks Sonnet for one cleaned reply and checks it. `correction` says why the last one was
    /// rejected.
    async fn transcribe_once(
        &mut self,
        correction: Option<&str>,
    ) -> Result<(TranscribedPage, Result<(), ReplyFault>), PageError> {
        let reply = self.services.transcribe(self.source, correction).await?;
        self.ledger.add(CallStep::Transcribe, &reply.usage);
        let mut page = reply.value;
        clean_reply(&mut page);
        let checked = check_reply(&page, self.text_layer_words);
        Ok((page, checked))
    }

    /// Asks Sonnet for the page, and once more with the fault's sentence if the reply breaks a
    /// rule. Returns the page to save and, when a second reply was asked for, why.
    ///
    /// A reply whose only fault is a figure's rectangle is good content, so it is never thrown
    /// away: that figure gets the whole page as its picture.
    async fn transcribe_checked(&mut self) -> Result<(TranscribedPage, Option<String>), PageError> {
        let (first, checked) = self.transcribe_once(None).await?;
        let first_fault = match checked {
            Ok(()) => return Ok((first, None)),
            Err(fault) => fault,
        };
        let reason = first_fault.to_string();
        let (second, checked) = self.transcribe_once(Some(&reason)).await?;
        let second_fault = match checked {
            Ok(()) => return Ok((second, Some(reason))),
            Err(fault) => fault,
        };
        let is_bad_bounds =
            |fault: &ReplyFault| matches!(fault, ReplyFault::BadFigureBounds { .. });
        if is_bad_bounds(&second_fault) {
            return Ok((second, Some(reason)));
        }
        // The first reply was fine but for its rectangle and the second came back worse.
        if is_bad_bounds(&first_fault) {
            return Ok((first, Some(reason)));
        }

        // The model has said twice that nothing is printed. A text layer can hold words that are
        // printed nowhere, so the page is accepted as blank.
        if matches!(first_fault, ReplyFault::NoPieces { .. })
            && matches!(second_fault, ReplyFault::NoPieces { .. })
        {
            return Ok((second, Some(reason)));
        }

        let saved_reply = self.partial_folder.join(REJECTED_REPLY_FILE);
        let save_error = |source| ContentError::Write {
            path: saved_reply.clone(),
            source,
        };
        let text =
            serde_json::to_string_pretty(&second).map_err(|error| save_error(error.into()))?;
        std::fs::write(&saved_reply, format!("{text}\n")).map_err(save_error)?;
        Err(PageError::ReplyRejected {
            fault: second_fault,
            saved_reply,
        })
    }
}

/// Converts one page whose files are already in its working folder, and renames that folder to
/// the page's finished name once everything is saved.
pub(super) async fn convert_page<S: PageServices>(
    services: &S,
    source: &PageSource,
    chapter_folder: &Path,
) -> Result<CallTally, PageError> {
    let partial_folder = chapter_folder.join(partial_page_folder_name(source.position));
    let mut run = PageRun {
        services,
        source,
        partial_folder: &partial_folder,
        text_layer_words: text_layer_word_count(&source.text_layer),
        ledger: Ledger::default(),
    };
    let (tags, math_check) = run.tag_and_check_math().await?;
    let mut reasons = route_reasons(&tags, math_check, run.text_layer_words);
    let written = run.write_by_route(&mut reasons).await?;
    let page = &written.page;
    let cut = cut_figures(&partial_folder, page).await;

    let reported_displayed_math =
        tags.math_notation || matches!(math_check, MathCheck::Block | MathCheck::Both);
    let has_formula = page
        .pieces
        .iter()
        .any(|piece| matches!(piece, TranscribedPiece::Formula { .. }));
    let conversion = Conversion {
        tags,
        math_check,
        route: written.route,
        route_reasons: reasons,
        checks: Checks {
            text_layer_words: run.text_layer_words,
            piece_words: piece_word_count(page),
            word_match: word_match(page, &source.text_layer),
            displayed_math_without_formula: reported_displayed_math && !has_formula,
            reply_retried: written.retry_reason.is_some(),
            retry_reason: written.retry_reason,
            whole_page_figures: cut.whole_page,
        },
        calls: run.ledger.records,
    };
    write_page(
        &partial_folder,
        source.position,
        page,
        &cut.images,
        conversion,
    )?;
    std::fs::remove_file(&source.image).map_err(|error| ContentError::Write {
        path: source.image.clone(),
        source: error,
    })?;
    let done_folder = chapter_folder.join(page_folder_name(source.position));
    std::fs::rename(&partial_folder, &done_folder).map_err(|source| ContentError::Write {
        path: done_folder,
        source,
    })?;
    Ok(run.ledger.tally)
}
