//! Converts a picture that stands alone, such as a chart on its own, into a saved folder: a copy
//! of the picture, the explanation of what it shows, and an index. Nothing is cut out of it,
//! because the picture already is the figure.

use std::path::{Path, PathBuf};

use super::checks::{ReplyFault, check_reply, clean_reply};
use super::page::call_record;
use super::reply::{TranscribedPage, TranscribedPiece};
use super::save::figure_file;
use super::services::{Answer, ImageServices, LiveImageServices};
use super::{ConvertError, sha256_hex, write_error};
use crate::content::{
    CallStep, ContentError, FORMAT_VERSION, IMAGE_EXPLANATION_FILE, ImageIndex,
    image_copy_file_name, image_folder,
};

/// The kinds of picture that can be read, by the extension of the file name in lower case.
const PICTURE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// A picture that stands alone, converted: where its files are and what was read from it.
#[derive(Debug, Clone, PartialEq)]
pub struct ConvertedImage {
    pub index: ImageIndex,
    /// The content of `figure.md`, without its closing newline: the explanation, then the words
    /// printed on the picture.
    pub explanation: String,
    /// The copy of the picture inside the folder, as an absolute path with every symbolic link
    /// resolved.
    pub picture: PathBuf,
}

/// Converts the picture with the real model, or returns what an earlier run saved. See
/// [`convert_image_with`].
///
/// # Errors
/// The same as [`convert_image_with`].
pub async fn convert_image(
    picture: &Path,
    output_root: &Path,
) -> Result<ConvertedImage, ConvertError> {
    convert_image_with(picture, output_root, &LiveImageServices).await
}

/// Converts a PNG or a JPEG that is one figure, and saves it under
/// `<output_root>/images/<the start of its SHA-256>/`, so the same bytes always land in the same
/// folder. A picture that was converted before is read back, and no paid call is made.
///
/// The model reads a copy of the picture inside that folder, never the person's own folder. Its
/// reply must pass the checks of a page and hold exactly one figure piece, and it is asked once
/// more when it does not. Pieces of other kinds and the rectangle of the figure are left out,
/// because nothing is cut out of the picture.
///
/// # Errors
/// - [`ConvertError::NotAPicture`] when the file name does not end in `.png`, `.jpg` or `.jpeg`
/// - [`ConvertError::PictureUnreadable`] when the file cannot be read
/// - [`ConvertError::Services`] when the paid call fails
/// - [`ConvertError::ImageReplyRejected`] when both replies broke a rule
/// - [`ConvertError::Content`] when the saved files cannot be read or written
pub async fn convert_image_with<S: ImageServices>(
    picture: &Path,
    output_root: &Path,
    services: &S,
) -> Result<ConvertedImage, ConvertError> {
    let extension = picture
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .filter(|extension| PICTURE_EXTENSIONS.contains(&extension.as_str()))
        .ok_or_else(|| ConvertError::NotAPicture {
            path: picture.to_path_buf(),
        })?;
    let bytes = std::fs::read(picture).map_err(|source| ConvertError::PictureUnreadable {
        path: picture.to_path_buf(),
        source,
    })?;
    let source_sha256 = sha256_hex(&bytes);
    let folder = output_root.join(image_folder(&source_sha256));
    if let Some(saved) = read_saved(&folder, &source_sha256)? {
        return Ok(saved);
    }

    let copy_name = image_copy_file_name(&extension.replace("jpeg", "jpg"));
    let copy = folder.join(&copy_name);
    std::fs::create_dir_all(&folder).map_err(write_error(&folder))?;
    std::fs::write(&copy, &bytes).map_err(write_error(&copy))?;

    let mut calls = Vec::new();
    let mut correction: Option<String> = None;
    let figure = loop {
        // SMELL: a call that fails here is reported in the words for services that "could not be
        // started", which is not what happened to it.
        let answer = services.transcribe(&copy, correction.as_deref()).await?;
        calls.push(call_record(CallStep::Transcribe, &answer.usage));
        match only_figure(answer) {
            Ok(figure) => break figure,
            Err(fault) if correction.is_none() => correction = Some(fault.to_string()),
            Err(fault) => {
                return Err(ConvertError::ImageReplyRejected {
                    picture: picture.to_path_buf(),
                    fault,
                });
            }
        }
    };

    let explanation = figure_file(&figure.explanation, &figure.printed_text)
        .trim()
        .to_owned();
    let explanation_file = folder.join(IMAGE_EXPLANATION_FILE);
    std::fs::write(&explanation_file, format!("{explanation}\n"))
        .map_err(write_error(&explanation_file))?;
    let index = ImageIndex {
        format_version: FORMAT_VERSION,
        source_file: file_name_of(picture),
        source_sha256,
        picture: copy_name,
        label: figure.label,
        caption: figure.caption,
        printed_text: figure.printed_text,
        calls,
    };
    index.write(&folder)?;
    Ok(ConvertedImage {
        picture: canonical(&folder)?.join(&index.picture),
        index,
        explanation,
    })
}

/// What the model said about the one figure of the picture.
struct LoneFigure {
    label: Option<String>,
    caption: Option<String>,
    printed_text: Vec<String>,
    explanation: String,
}

/// The one figure of a reply that passes the checks of a page. A bad rectangle is not a fault
/// here, because nothing is cut out, and pieces of other kinds are left out.
fn only_figure(answer: Answer<TranscribedPage>) -> Result<LoneFigure, ReplyFault> {
    let mut page = answer.value;
    clean_reply(&mut page);
    match check_reply(&page, 0) {
        Ok(()) | Err(ReplyFault::BadFigureBounds { .. }) => {}
        Err(fault) => return Err(fault),
    }
    let figures: Vec<LoneFigure> = page
        .pieces
        .into_iter()
        .filter_map(|piece| match piece {
            TranscribedPiece::Figure {
                label,
                caption,
                printed_text,
                explanation,
                ..
            } => Some(LoneFigure {
                label,
                caption,
                printed_text,
                explanation,
            }),
            _ => None,
        })
        .collect();
    let found = figures.len();
    <[LoneFigure; 1]>::try_from(figures)
        .map(|[figure]| figure)
        .map_err(|_| ReplyFault::NotOneFigure { found })
}

/// What an earlier run saved, when the folder holds a whole conversion of these bytes. A folder
/// that is missing, was made from other bytes or at another format version, or lacks one of its
/// files counts as not converted.
fn read_saved(folder: &Path, source_sha256: &str) -> Result<Option<ConvertedImage>, ConvertError> {
    let index = match ImageIndex::read(folder) {
        Ok(index) => index,
        Err(ContentError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(ContentError::Parse { .. }) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let copy = folder.join(&index.picture);
    let explanation_file = folder.join(IMAGE_EXPLANATION_FILE);
    if index.format_version != FORMAT_VERSION
        || index.source_sha256 != source_sha256
        || !copy.is_file()
    {
        return Ok(None);
    }
    let explanation = match std::fs::read_to_string(&explanation_file) {
        Ok(text) => text.trim().to_owned(),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ContentError::Read {
                path: explanation_file,
                source,
            }
            .into());
        }
    };
    Ok(Some(ConvertedImage {
        picture: canonical(folder)?.join(&index.picture),
        index,
        explanation,
    }))
}

fn canonical(folder: &Path) -> Result<PathBuf, ContentError> {
    std::fs::canonicalize(folder).map_err(|source| ContentError::Read {
        path: folder.to_path_buf(),
        source,
    })
}

fn file_name_of(picture: &Path) -> String {
    picture
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Mutex;

    use super::*;
    use crate::content::PageBox;
    use crate::convert::reply::{TranscribedPage, TranscribedPiece};
    use crate::convert::services::{Answer, CallUsage, ServiceError};

    fn explanation() -> String {
        vec!["word"; 70].join(" ")
    }

    fn figure(number: u32) -> TranscribedPiece {
        TranscribedPiece::Figure {
            number,
            label: Some("Figure 24-12".to_owned()),
            caption: Some("FTSE 100 volatility surface".to_owned()),
            printed_text: vec![
                "Exercise price".to_owned(),
                "Months to expiration".to_owned(),
            ],
            bounds: PageBox {
                left: 0,
                top: 0,
                right: 1000,
                bottom: 1000,
            },
            explanation: explanation(),
        }
    }

    fn page(pieces: Vec<TranscribedPiece>) -> TranscribedPage {
        TranscribedPage {
            printed_page_number: None,
            running_header: None,
            pieces,
            discusses: Vec::new(),
            starts_mid_sentence: false,
            ends_mid_sentence: false,
        }
    }

    /// Answers each call with the next page of the list, and keeps what it was asked.
    struct Scripted {
        pages: Mutex<Vec<TranscribedPage>>,
        asked: Mutex<Vec<(std::path::PathBuf, Option<String>)>>,
    }

    impl Scripted {
        fn new(mut pages: Vec<TranscribedPage>) -> Scripted {
            pages.reverse();
            Scripted {
                pages: Mutex::new(pages),
                asked: Mutex::new(Vec::new()),
            }
        }
    }

    impl ImageServices for Scripted {
        async fn transcribe(
            &self,
            picture: &Path,
            correction: Option<&str>,
        ) -> Result<Answer<TranscribedPage>, ServiceError> {
            self.asked
                .lock()
                .unwrap()
                .push((picture.to_path_buf(), correction.map(str::to_owned)));
            let value = self.pages.lock().unwrap().pop().expect("a page is left");
            Ok(Answer {
                value,
                usage: CallUsage {
                    model: "stand-in".to_owned(),
                    cost_usd: 0.0,
                    output_tokens: 1,
                    thinking_tokens: 0,
                    seconds: 0.0,
                },
            })
        }
    }

    fn sample_picture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/images/volatility-surface.png")
    }

    #[tokio::test]
    async fn a_reply_with_two_figures_is_asked_for_again_and_a_second_fault_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let two = page(vec![figure(1), figure(2)]);
        let services = Scripted::new(vec![two.clone(), page(vec![figure(1)])]);
        convert_image_with(&sample_picture(), root.path(), &services)
            .await
            .unwrap();
        {
            let asked = services.asked.lock().unwrap();
            assert_eq!(asked.len(), 2);
            assert!(
                asked[1].1.as_deref().unwrap().contains("one figure"),
                "{:?}",
                asked[1]
            );
            for (given, _) in asked.iter() {
                assert!(
                    given.starts_with(root.path()) && given.ends_with("picture.png"),
                    "the model was given {given:?}, which is not the copy under the output root"
                );
            }
        }

        let other_root = tempfile::tempdir().unwrap();
        let services = Scripted::new(vec![two.clone(), two]);
        let error = convert_image_with(&sample_picture(), other_root.path(), &services)
            .await
            .unwrap_err();
        assert!(
            matches!(error, ConvertError::ImageReplyRejected { .. }),
            "{error:?}"
        );
        assert!(
            !other_root
                .path()
                .join("images")
                .read_dir()
                .unwrap()
                .any(|entry| { entry.unwrap().path().join("image.json").exists() })
        );
    }
}
