//! What a chapter run reports, as it works and when it ends. Routes, piece counts and the pages
//! to look at are always read from the saved `page.json` files, so a fresh run and a re-run print
//! the same.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::services::CallUsage;
use crate::content::{
    ChapterIndex, ContentError, Conversion, FigureImage, ImageShows, PageIndex, PieceDetail, Route,
    page_folder_name,
};

/// A Sonnet-written page whose copied words match the text layer less well than this, in either
/// direction, is listed as a page to check. The text layer is unreliable around math, charts and
/// tables, so a middling match there is normal and is never a failure.
const LOW_WORD_MATCH: f64 = 0.60;

/// A figure picture that covers more than this share of its page is listed as a page to check: a
/// figure is rarely that big, so a rectangle that large is more likely to be wrong.
const MOST_OF_THE_PAGE_PERCENT: i64 = 80;

/// What a chapter run tells its caller while it works, as it happens.
#[derive(Debug, Clone, PartialEq)]
pub enum PageProgress {
    /// The pages are known, before any page is cut out or paid for: `total` in the PDF, and
    /// `done_before` of them saved by an earlier run. It comes once, before the first page is cut
    /// out, on every run of a chapter that is not finished.
    Pages { total: u32, done_before: u32 },
    /// One more page is saved. `calls` are the paid calls of that page. Pages end in any order.
    PageDone { position: u32, calls: CallTally },
    /// A page failed. No new page starts; the pages that are running finish first.
    PageFailed { position: u32 },
}

/// What the calls of one model used, as `claude` reported it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ModelCalls {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: f64,
}

impl ModelCalls {
    fn add(&mut self, other: &ModelCalls) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.cost_usd += other.cost_usd;
    }
}

/// The paid calls a run made. A corrected retry counts as a call of its own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CallTally {
    pub tag: u32,
    pub math_check: u32,
    pub copy: u32,
    pub transcribe: u32,
    /// The calls of each model that answered, by the model's name.
    pub by_model: BTreeMap<String, ModelCalls>,
}

impl CallTally {
    pub(super) fn add(&mut self, other: &CallTally) {
        self.tag += other.tag;
        self.math_check += other.math_check;
        self.copy += other.copy;
        self.transcribe += other.transcribe;
        for (model, calls) in &other.by_model {
            self.by_model.entry(model.clone()).or_default().add(calls);
        }
    }

    /// Adds what one call used to the line of its model. The call itself is counted by its step.
    pub(super) fn add_usage(&mut self, usage: &CallUsage) {
        let calls = ModelCalls {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            cost_usd: usage.cost_usd,
        };
        self.by_model
            .entry(usage.model.clone())
            .or_default()
            .add(&calls);
    }

    fn total(&self) -> u32 {
        self.tag + self.math_check + self.copy + self.transcribe
    }
}

/// A page written by hand is in none of these counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RouteCounts {
    pub haiku_copy: u32,
    pub sonnet: u32,
    pub haiku_then_sonnet: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PieceCounts {
    pub heading: u32,
    pub text: u32,
    pub formula: u32,
    pub figure: u32,
    pub table: u32,
    pub footnote: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutOfSequence {
    pub position: u32,
    pub shown: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageToCheck {
    pub position: u32,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversionSummary {
    pub chapter_folder: PathBuf,
    pub page_count: u32,
    pub converted_now: u32,
    pub already_done: u32,
    pub routes: RouteCounts,
    pub pieces: PieceCounts,
    pub calls: CallTally,
    pub out_of_sequence: Vec<OutOfSequence>,
    pub pages_to_check: Vec<PageToCheck>,
}

impl ConversionSummary {
    /// Builds the summary from the saved pages. `calls` is what this run spent and
    /// `converted_now` how many pages it converted.
    pub(super) fn from_folder(
        chapter_folder: &Path,
        converted_now: u32,
        calls: CallTally,
    ) -> Result<Self, ContentError> {
        let chapter = ChapterIndex::read(chapter_folder)?;
        let mut summary = Self {
            chapter_folder: chapter_folder.to_path_buf(),
            page_count: chapter.page_count,
            converted_now,
            already_done: chapter.page_count.saturating_sub(converted_now),
            routes: RouteCounts::default(),
            pieces: PieceCounts::default(),
            calls,
            out_of_sequence: Vec::new(),
            pages_to_check: Vec::new(),
        };
        let mut last_numbered: Option<(u32, u32)> = None;
        for position in 1..=chapter.page_count {
            let page = PageIndex::read(&chapter_folder.join(page_folder_name(position)))?;
            summary.count_pieces(&page);
            // A page written by hand has no record of how it was made, so it is in no route.
            let reasons = match &page.conversion {
                Some(conversion) => {
                    summary.count_route(conversion.route);
                    reasons_to_check(&page, conversion)
                }
                None => Vec::new(),
            };
            if !reasons.is_empty() {
                summary
                    .pages_to_check
                    .push(PageToCheck { position, reasons });
            }
            // Pages with no number, or one that is not a whole number, are skipped.
            if let Some(shown) = &page.printed_page_number
                && let Ok(number) = shown.parse::<u32>()
            {
                if let Some((last_number, last_position)) = last_numbered
                    && u64::from(number)
                        != u64::from(last_number) + u64::from(position - last_position)
                {
                    summary.out_of_sequence.push(OutOfSequence {
                        position,
                        shown: shown.clone(),
                    });
                }
                last_numbered = Some((number, position));
            }
        }
        Ok(summary)
    }

    fn count_route(&mut self, route: Route) {
        match route {
            Route::HaikuCopy => self.routes.haiku_copy += 1,
            Route::Sonnet => self.routes.sonnet += 1,
            Route::HaikuThenSonnet => self.routes.haiku_then_sonnet += 1,
        }
    }

    fn count_pieces(&mut self, page: &PageIndex) {
        for piece in &page.pieces {
            let count = match piece.detail {
                PieceDetail::Heading { .. } => &mut self.pieces.heading,
                PieceDetail::Text { .. } => &mut self.pieces.text,
                PieceDetail::Formula { .. } => &mut self.pieces.formula,
                PieceDetail::Figure { .. } => &mut self.pieces.figure,
                PieceDetail::Table { .. } => &mut self.pieces.table,
                PieceDetail::Footnote { .. } => &mut self.pieces.footnote,
            };
            *count += 1;
        }
    }
}

fn reasons_to_check(page: &PageIndex, conversion: &Conversion) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    let word_match = conversion.checks.word_match;
    let written_by_sonnet = conversion.route != Route::HaikuCopy;
    if written_by_sonnet
        && (word_match.piece_words_in_text_layer < LOW_WORD_MATCH
            || word_match.text_layer_words_in_pieces < LOW_WORD_MATCH)
    {
        reasons.push("low word match");
    }
    if conversion.checks.displayed_math_without_formula {
        reasons.push("displayed math reported, no formula piece");
    }
    if !conversion.checks.whole_page_figures.is_empty() {
        reasons.push("figure image is the whole page");
    }
    let images: Vec<&FigureImage> = page
        .pieces
        .iter()
        .filter_map(|piece| match &piece.detail {
            PieceDetail::Figure { image, .. } => image.as_ref(),
            _ => None,
        })
        .collect();
    let has_text = page
        .pieces
        .iter()
        .any(|piece| matches!(piece.detail, PieceDetail::Text { .. }));
    if has_text && images.iter().any(|image| covers_most_of_the_page(image)) {
        reasons.push("figure image is most of the page");
    }
    if images.iter().any(|image| {
        matches!(
            image.shows,
            ImageShows::Figure {
                holds_body_text: true,
                ..
            }
        )
    }) {
        reasons.push("figure image holds body text");
    }
    if images.iter().any(|image| {
        matches!(
            image.shows,
            ImageShows::Figure {
                unchecked: true,
                ..
            }
        )
    }) {
        reasons.push("figure image was not checked against the page's text");
    }
    reasons
}

fn covers_most_of_the_page(image: &FigureImage) -> bool {
    // A rectangle is in thousandths of the page each way, so the whole page is a million.
    const WHOLE_PAGE: i64 = 1_000_000;
    let ImageShows::Figure { cut: Some(cut), .. } = image.shows else {
        return false;
    };
    let area = i64::from(cut.right - cut.left) * i64::from(cut.bottom - cut.top);
    area * 100 > MOST_OF_THE_PAGE_PERCENT * WHOLE_PAGE
}

impl fmt::Display for ConversionSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let routes = &self.routes;
        let pieces = &self.pieces;
        let calls = &self.calls;
        writeln!(
            formatter,
            "chapter folder: {}",
            self.chapter_folder.display()
        )?;
        writeln!(
            formatter,
            "pages: {} ({} converted now, {} already done)",
            self.page_count, self.converted_now, self.already_done
        )?;
        writeln!(
            formatter,
            "routes: haiku-copy {}, sonnet {}, haiku-then-sonnet {}",
            routes.haiku_copy, routes.sonnet, routes.haiku_then_sonnet
        )?;
        writeln!(
            formatter,
            "pieces: heading {}, text {}, formula {}, figure {}, table {}, footnote {}",
            pieces.heading,
            pieces.text,
            pieces.formula,
            pieces.figure,
            pieces.table,
            pieces.footnote
        )?;
        // No cost is printed: this crate has no table of prices, and the figure that `claude`
        // reports is not the one the programs that do have the table print.
        writeln!(
            formatter,
            "calls this run: {} (tag {}, math check {}, copy {}, transcribe {})",
            calls.total(),
            calls.tag,
            calls.math_check,
            calls.copy,
            calls.transcribe
        )?;
        // SMELL: the core crate keeps its own copy of these words, and this crate does not depend
        // on it, so a change to one must be made in both.
        for (model, used) in &calls.by_model {
            writeln!(
                formatter,
                "tokens of {model}: {} in, {} out, {} cache read, {} cache write",
                used.input_tokens,
                used.output_tokens,
                used.cache_read_tokens,
                used.cache_write_tokens
            )?;
        }
        if self.out_of_sequence.is_empty() {
            writeln!(formatter, "printed page numbers: in sequence")?;
        } else {
            let listed: Vec<String> = self
                .out_of_sequence
                .iter()
                .map(|page| format!("page {} shows {}", page.position, page.shown))
                .collect();
            writeln!(
                formatter,
                "printed page numbers out of sequence: {}",
                listed.join(", ")
            )?;
        }
        if self.pages_to_check.is_empty() {
            write!(formatter, "pages to check: none")
        } else {
            let listed: Vec<String> = self
                .pages_to_check
                .iter()
                .map(|page| format!("page {} ({})", page.position, page.reasons.join(", ")))
                .collect();
            write!(formatter, "pages to check: {}", listed.join(", "))
        }
    }
}
