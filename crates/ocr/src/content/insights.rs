//! What the model wrote about a whole chapter, saved in the chapter's folder so that a program
//! that shows the chapter can read it back without asking a model.

use std::path::Path;

use rag_core::Category;
use serde::{Deserialize, Serialize};

use super::index::{read_json, write_json};
use super::{ContentError, DocumentName, INSIGHTS_FILE};

/// The insights, the takeaways and the cheat sheet of one chapter, saved as `insights.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ChapterInsights {
    /// The model that wrote it, as its agent names it.
    pub model: String,
    /// Which wording of the question asked for it. Another version means another way of asking.
    pub prompt_version: String,
    pub insights: Vec<Insight>,
    /// One line of Markdown each.
    pub takeaways: Vec<String>,
    /// Markdown with short headed sections, formulas and small tables.
    pub cheat_sheet: String,
}

/// One thing the model found worth knowing about the chapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Insight {
    /// A short Markdown paragraph. Formulas are written between `$` or `$$`.
    pub text: String,
    /// The pages the insight rests on, counted from 1 the way a stored item counts its page.
    pub pages: Vec<u32>,
}

impl ChapterInsights {
    /// `Ok(None)` when the folder holds no `insights.json`, so a chapter that has none yet is not
    /// an error.
    ///
    /// # Errors
    /// - [`ContentError::Read`] when the file is there and cannot be read
    /// - [`ContentError::Parse`] when it is not valid json of this shape
    pub fn read(chapter_folder: &Path) -> Result<Option<ChapterInsights>, ContentError> {
        match read_json(&chapter_folder.join(INSIGHTS_FILE)) {
            Ok(insights) => Ok(Some(insights)),
            Err(ContentError::Read { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Writes the file whole or not at all, so a killed run never leaves half of it.
    ///
    /// # Errors
    /// [`ContentError::Write`] when the file cannot be written.
    pub fn write(&self, chapter_folder: &Path) -> Result<(), ContentError> {
        write_json(&chapter_folder.join(INSIGHTS_FILE), self)
    }
}

/// Whether a document of this name, in a media of this category, gets insights. A part of a book
/// that is not a chapter, such as its index or its glossary, gets none: it has no argument to
/// summarise. A chapter does, and so does a document of another media, such as a paper. A document
/// whose category is not known counts as one of another media.
pub fn gets_insights(name: &DocumentName, category: Option<Category>) -> bool {
    !matches!(
        (name, category),
        (DocumentName::Title(_), Some(Category::Book))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ChapterInsights {
        ChapterInsights {
            model: "claude-sonnet-5-5".to_owned(),
            prompt_version: "1".to_owned(),
            insights: vec![Insight {
                text: "The price is $S e^{-qT}$ discounted.".to_owned(),
                pages: vec![1, 3],
            }],
            takeaways: vec!["Volatility is not constant.".to_owned()],
            cheat_sheet: "### Terms\n\n| a | b |\n|---|---|\n| 1 | 2 |".to_owned(),
        }
    }

    #[test]
    fn insights_round_trip_and_a_folder_without_them_reads_none() {
        let folder = tempfile::tempdir().unwrap();
        assert_eq!(ChapterInsights::read(folder.path()).unwrap(), None);

        sample().write(folder.path()).unwrap();
        assert_eq!(
            ChapterInsights::read(folder.path()).unwrap(),
            Some(sample())
        );
        let on_disk = std::fs::read_to_string(folder.path().join(INSIGHTS_FILE)).unwrap();
        assert!(on_disk.contains("\"prompt-version\""), "{on_disk}");
        assert!(on_disk.contains("\"cheat-sheet\""), "{on_disk}");

        std::fs::write(folder.path().join(INSIGHTS_FILE), "not json").unwrap();
        let error = ChapterInsights::read(folder.path()).unwrap_err();
        assert!(matches!(error, ContentError::Parse { .. }), "{error:?}");
    }
}
