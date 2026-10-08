//! Checks a document of a media before it is ingested, and ingests it.

mod preflight;
mod run;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use ocr::{ChapterJob, MediaDocument};

use crate::backend::live::{LiveContext, Services};
use crate::contract::{Failure, FailureKind, IngestRequest};

pub use preflight::preflight;
pub use run::run;

const PDF_START: &[u8] = b"%PDF-";

/// These checks cost nothing, and a check and a start share them. Nothing is written and no
/// store is asked. The name of the document comes with the request as the person typed it, so
/// the PDF may have any file name.
fn chapter_job<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<ChapterJob, Failure> {
    let document = MediaDocument {
        media_title: ingest.media.clone(),
        name: (&ingest.name).into(),
    };
    let job = ChapterJob::new(document, &ingest.pdf, &cx.config().content_folder)
        .map_err(|error| cx.failure(error))?;
    check_is_a_pdf(&ingest.pdf)?;
    Ok(job)
}

fn file_name_of(pdf: &Path) -> String {
    pdf.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The size is not limited here.
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
