//! Finds the fenced listings in the text of a section and checks that the Python ones compile.

use std::io;
use std::process::{Command, Stdio};
use std::sync::Once;

use rag_core::error_chain;

/// A listing as it stands between its fences.
pub(super) struct Listing {
    /// As written after the opening fence, such as `python`. Empty when the listing has none.
    language: String,
    code: String,
}

/// A listing whose closing fence has not been seen yet.
struct OpenListing<'a> {
    fence: &'a str,
    language: String,
    lines: Vec<&'a str>,
}

impl<'a> OpenListing<'a> {
    fn starting_at(line: &'a str) -> Option<Self> {
        let backticks = line
            .chars()
            .take_while(|&character| character == '`')
            .count();
        (backticks >= 3).then(|| OpenListing {
            fence: &line[..backticks],
            language: line[backticks..].trim().to_owned(),
            lines: Vec::new(),
        })
    }

    fn into_listing(self) -> Listing {
        Listing {
            language: self.language,
            code: self.lines.join("\n"),
        }
    }
}

/// The lines of `markdown` outside every listing, and the listings. The renderer writes a fence
/// longer than any run of backticks inside the listing, so a listing ends at the first line that
/// is its fence and nothing else.
pub(super) fn split_listings(markdown: &str) -> (String, Vec<Listing>) {
    let mut prose = Vec::new();
    let mut listings = Vec::new();
    let mut open: Option<OpenListing> = None;
    for line in markdown.split('\n') {
        match open.take() {
            Some(listing) if line == listing.fence => listings.push(listing.into_listing()),
            Some(mut listing) => {
                listing.lines.push(line);
                open = Some(listing);
            }
            None => match OpenListing::starting_at(line) {
                Some(listing) => open = Some(listing),
                None => prose.push(line),
            },
        }
    }
    listings.extend(open.map(OpenListing::into_listing));
    (prose.join("\n"), listings)
}

#[derive(thiserror::Error, Debug)]
enum PythonError {
    #[error("python3 is not on the PATH")]
    NotInstalled,

    #[error("could not make a temporary folder")]
    TemporaryFolder(#[source] io::Error),

    #[error("could not write the listing to a temporary file")]
    WriteListing(#[source] io::Error),

    #[error("could not run python3")]
    RunPython(#[source] io::Error),
}

/// Whether every Python listing compiles, or none when there is no Python listing or Python
/// cannot be run. A listing is compiled and never run.
pub(super) fn python_compiles(listings: &[Listing]) -> Option<bool> {
    let python_listings: Vec<&Listing> = listings
        .iter()
        .filter(|listing| listing.language == "python")
        .collect();
    if python_listings.is_empty() {
        return None;
    }
    let results: Result<Vec<bool>, PythonError> = python_listings
        .iter()
        .map(|listing| compiles(&listing.code))
        .collect();
    match results {
        Ok(results) => Some(results.into_iter().all(|listing_compiles| listing_compiles)),
        Err(PythonError::NotInstalled) => {
            static SAID_SO: Once = Once::new();
            SAID_SO.call_once(|| {
                tracing::warn!(
                    "python3 is not on the PATH, so the Python listings are not checked"
                );
            });
            None
        }
        Err(error) => {
            tracing::warn!("a Python listing was not checked: {}", error_chain(&error));
            None
        }
    }
}

fn compiles(code: &str) -> Result<bool, PythonError> {
    // The file gets a folder of its own, because `py_compile` writes its cache beside it.
    let folder = tempfile::tempdir().map_err(PythonError::TemporaryFolder)?;
    let file = folder.path().join("listing.py");
    std::fs::write(&file, code).map_err(PythonError::WriteListing)?;
    let status = Command::new("python3")
        // `-I` keeps Python from reading the current folder and the `PYTHON` variables.
        .args(["-I", "-m", "py_compile"])
        .arg(&file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => PythonError::NotInstalled,
            _ => PythonError::RunPython(source),
        })?;
    // SMELL: a python3 that starts but cannot compile anything, such as a broken install, marks
    // every listing as one that does not compile.
    Ok(status.success())
}
