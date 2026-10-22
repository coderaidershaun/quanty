//! The `export_insights` tool: the saved insights, takeaways and cheat sheet of one document,
//! written to a file that stands alone. Every check runs before anything is written.

use std::path::{Path, PathBuf};

use export::{Format, InsightsSheet, SheetInsight, insights_document};
use rag_core::{Config, DocId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::task::JoinError;

use crate::insights::{InsightsToolError, Saved, saved_insights};
use crate::library::{find_chapter, parse_document_id};

/// What an agent gives `export_insights`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct ExportInsightsArgs {
    /// From `list_documents` or a search result.
    document_id: String,
    /// `pdf`, `markdown` or `html`.
    format: String,
    /// The absolute path of the file to write, on the machine the server runs on, ending in
    /// `.pdf`, `.md` or `.html` as `format` is. Its folder must exist.
    path: String,
    /// True to replace a file that is at `path`. False when left out: a file that is there is
    /// kept and the call fails.
    overwrite: Option<bool>,
}

/// The file that was written.
#[derive(Serialize, JsonSchema)]
pub(crate) struct ExportResult {
    document_id: String,
    /// `pdf`, `markdown` or `html`.
    format: String,
    /// The path that was given.
    path: String,
    /// The size of the file.
    bytes: u64,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum ExportToolError {
    #[error("`format` must be `pdf`, `markdown` or `html`, and {given:?} is not")]
    Format { given: String },

    #[error("`path` must be an absolute path, and {} is not", path.display())]
    NotAbsolute { path: PathBuf },

    #[error("`path` must end with .{extension} for this format, and {} does not", path.display())]
    Extension {
        path: PathBuf,
        extension: &'static str,
    },

    #[error("the folder {} does not exist; give a path in a folder that exists", folder.display())]
    NoFolder { folder: PathBuf },

    #[error("{} is there already; give another path, or `overwrite: true` to replace it", path.display())]
    Exists { path: PathBuf },

    #[error(
        "no insights are saved for the document {id}; call `write_insights` with this `document_id` to write them, which is paid, then export again"
    )]
    NotWrittenYet { id: DocId },

    #[error(
        "the document {id} is a part of a book that is not a chapter, which gets no insights, so there is nothing to export"
    )]
    BookPart { id: DocId },

    #[error(transparent)]
    Insights(Box<InsightsToolError>),

    #[error("could not write the file")]
    Export(#[source] export::ExportError),

    #[error("the file {} was written, but its size could not be read", path.display())]
    Length {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("the export stopped without an answer; export again")]
    Stopped(#[from] JoinError),
}

impl From<InsightsToolError> for ExportToolError {
    fn from(error: InsightsToolError) -> ExportToolError {
        ExportToolError::Insights(Box::new(error))
    }
}

/// Writes the saved insights of the document that the id names to `args.path`.
///
/// # Errors
/// A format, a path or a document id that is refused, a file that is at the path while
/// `overwrite` is not true, a document with no saved insights, and a file that cannot be written.
pub(crate) async fn export_insights(
    config: &Config,
    args: ExportInsightsArgs,
) -> Result<ExportResult, ExportToolError> {
    let word = args.format.trim().to_ascii_lowercase();
    let format = format_of(&word).ok_or_else(|| ExportToolError::Format {
        given: args.format.clone(),
    })?;
    let path = PathBuf::from(&args.path);
    check_path(&path, format, args.overwrite == Some(true))?;
    let id = parse_document_id(&args.document_id).map_err(InsightsToolError::from)?;
    let entry = find_chapter(&config.content_folder, id).map_err(InsightsToolError::from)?;
    let saved = match saved_insights(config, id, &entry).await? {
        Saved::Written(saved) => saved,
        Saved::NotWrittenYet => return Err(ExportToolError::NotWrittenYet { id }),
        Saved::NoneForBookPart => return Err(ExportToolError::BookPart { id }),
    };
    let sheet = InsightsSheet {
        document_title: rag_ingestion::document_title(&entry.index),
        model: saved.model,
        insights: saved
            .insights
            .into_iter()
            .map(|insight| SheetInsight {
                text: insight.text,
                pages: insight.pages,
            })
            .collect(),
        takeaways: saved.takeaways,
        cheat_sheet: saved.cheat_sheet,
    };
    let document = insights_document(&sheet);
    let target = path.clone();
    // The file is checked before and written after, so one that appears in between is replaced.
    tokio::task::spawn_blocking(move || export::write(&document, format, &target))
        .await?
        .map_err(ExportToolError::Export)?;
    let bytes = std::fs::metadata(&path)
        .map_err(|source| ExportToolError::Length {
            path: path.clone(),
            source,
        })?
        .len();
    Ok(ExportResult {
        document_id: id.to_string(),
        format: word,
        path: args.path,
        bytes,
    })
}

fn format_of(word: &str) -> Option<Format> {
    match word {
        "pdf" => Some(Format::Pdf),
        "markdown" => Some(Format::Markdown),
        "html" => Some(Format::Html),
        _ => None,
    }
}

/// The path rules, in the order an agent can mend them: absolute, the extension of the format, a
/// folder that exists, and no file there unless it may be replaced.
fn check_path(path: &Path, format: Format, may_replace: bool) -> Result<(), ExportToolError> {
    if !path.is_absolute() {
        return Err(ExportToolError::NotAbsolute {
            path: path.to_owned(),
        });
    }
    let has_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()));
    if !has_extension {
        return Err(ExportToolError::Extension {
            path: path.to_owned(),
            extension: format.extension(),
        });
    }
    let folder = path.parent().unwrap_or(path);
    if !folder.is_dir() {
        return Err(ExportToolError::NoFolder {
            folder: folder.to_owned(),
        });
    }
    // A link that points nowhere counts as a file that is there.
    if !may_replace && path.symlink_metadata().is_ok() {
        return Err(ExportToolError::Exists {
            path: path.to_owned(),
        });
    }
    Ok(())
}
