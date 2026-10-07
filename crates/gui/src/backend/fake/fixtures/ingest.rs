//! The steps of an ingest, its report and the way it can fail.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::contract::{
    ChapterLabel, ChapterState, DocId, Failure, FailureKind, IngestReport, IngestRequest,
    ItemCounts, PageToCheck, Preflight,
};

/// The chapter that the two ingest scenes check as they open: the form as the page draws it, so
/// the page leaves it alone.
pub(in crate::backend::fake) fn request() -> IngestRequest {
    IngestRequest {
        pdf: PathBuf::from("/books/option-volatility/chapter-3-greeks.pdf"),
        book: "Option Volatility and Pricing".to_owned(),
        author: Some("Sheldon Natenberg".to_owned()),
        tags: vec!["options".to_owned(), "volatility".to_owned()],
    }
}

fn file_name(pdf: &Path) -> String {
    pdf.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

/// Reads `chapter-<number>-<name>.pdf` the way the real check does: the number is all digits and
/// the name is its words, each with a capital letter. The fake may not name the crate that
/// reads it for real, so the rule is written out here.
fn parse(name: &str) -> Option<ChapterLabel> {
    let stem = name.strip_prefix("chapter-")?.strip_suffix(".pdf")?;
    let (digits, words) = stem.split_once('-')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let words: Vec<String> = words
        .split('-')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(letters).collect()
            })
        })
        .collect();
    if words.is_empty() {
        return None;
    }
    Some(ChapterLabel {
        number: digits.parse().ok()?,
        name: words.join(" "),
    })
}

fn chapter_of(pdf: &Path) -> Result<ChapterLabel, Failure> {
    let name = file_name(pdf);
    parse(&name).ok_or_else(|| {
        Failure::new(
            FailureKind::BadFile,
            format!("{name} is not named chapter-<number>-<name>.pdf"),
        )
        .with_hint(format!(
            "{name} is not named chapter-<number>-<name>.pdf. Rename the file, for example chapter-3-greeks.pdf."
        ))
    })
}

/// What the free check finds: a new chapter with a name that fits, or a file that is refused.
pub(in crate::backend::fake) fn preflight(request: &IngestRequest) -> Result<Preflight, Failure> {
    Ok(Preflight {
        chapter: chapter_of(&request.pdf)?,
        pages: None,
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    })
}

/// The report of a run that stored the chapter. The numbers are made up.
pub(in crate::backend::fake) fn report(request: &IngestRequest) -> IngestReport {
    let chapter = chapter_of(&request.pdf).unwrap_or_default();
    IngestReport {
        doc: DocId(Uuid::from_u128(9)),
        title: format!(
            "{}, chapter {}: {}",
            request.book, chapter.number, chapter.name
        ),
        pages: 12,
        items: ItemCounts {
            chunks: 31,
            formulas: 8,
            figures: 3,
            tables: 2,
        },
        concepts_created: 14,
        concepts_linked: 9,
        skipped_items: 1,
        cost_usd: Some(1.84),
        pages_to_check: vec![PageToCheck {
            page: 7,
            reasons: vec!["A figure may be cut short.".to_owned()],
        }],
    }
}
