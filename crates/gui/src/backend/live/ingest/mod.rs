//! Checks a chapter before it is ingested, and ingests it.

mod preflight;
mod run;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use ocr::ChapterJob;
use ocr::content::parse_chapter_file_name;

use crate::backend::live::{LiveContext, Services};
use crate::contract::{ChapterLabel, Failure, FailureKind, IngestRequest};

pub use preflight::preflight;
pub use run::run;

/// What every PDF starts with.
const PDF_START: &[u8] = b"%PDF-";

/// The chapter the request names, and the job that converts it: the checks that cost nothing and
/// that a check and a start share. Nothing is written and no store is asked.
fn chapter_job<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<(ChapterLabel, ChapterJob), Failure> {
    let chapter =
        parse_chapter_file_name(&file_name_of(&ingest.pdf)).map_err(|error| cx.failure(error))?;
    let label = ChapterLabel {
        number: chapter.number,
        name: chapter.name,
    };
    let job = ChapterJob::new(&ingest.book, &ingest.pdf, &cx.config().content_folder)
        .map_err(|error| cx.failure(error))?;
    check_is_a_pdf(&ingest.pdf)?;
    Ok((label, job))
}

fn file_name_of(pdf: &Path) -> String {
    pdf.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Passes a file that can be read and starts like a PDF. The size is not limited here.
fn check_is_a_pdf(pdf: &Path) -> Result<(), Failure> {
    let name = file_name_of(pdf);
    let refused = |problem: &str, detail: String| {
        let hint = format!("{name} {problem}. Choose a PDF that opens in a PDF reader.");
        Failure::new(FailureKind::BadFile, detail).with_hint(hint)
    };
    let cannot_read = |error: std::io::Error| {
        refused(
            "cannot be read",
            format!("could not read {}: {error}", pdf.display()),
        )
    };
    let file = File::open(pdf).map_err(cannot_read)?;
    if !file.metadata().map_err(cannot_read)?.is_file() {
        let detail = format!("{} is not a file", pdf.display());
        return Err(refused("is not a file", detail));
    }
    let mut start = Vec::new();
    file.take(PDF_START.len() as u64)
        .read_to_end(&mut start)
        .map_err(cannot_read)?;
    if start != PDF_START {
        let detail = format!("{} does not start with %PDF-", pdf.display());
        return Err(refused("is not a PDF", detail));
    }
    Ok(())
}
