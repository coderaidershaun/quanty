//! Reads how well each page of a section was copied from the book, as the conversion measured it.

use ocr::content::{PageIndex, RouteReason, page_folder_name};
use rag_core::error_chain;

use crate::section::Section;

/// The lowest share, over the pages of the section, of the copied words that the page's own text
/// layer also holds. It is none when a page was written by hand and so has no conversion record,
/// when a page has no text layer to compare with, or when a page cannot be read.
pub(super) fn page_score(section: &Section) -> Option<f64> {
    let mut lowest: Option<f64> = None;
    for &position in &section.pages {
        let folder = section.document.folder.join(page_folder_name(position));
        let index = match PageIndex::read(&folder) {
            Ok(index) => index,
            Err(error) => {
                tracing::warn!(
                    "the pages of section {} were not scored, because {} cannot be read: {}",
                    section.id,
                    folder.display(),
                    error_chain(&error)
                );
                return None;
            }
        };
        let conversion = index.conversion?;
        if conversion
            .route_reasons
            .contains(&RouteReason::TextLayerAlmostEmpty)
        {
            return None;
        }
        let score = conversion.checks.word_match.piece_words_in_text_layer;
        lowest = Some(lowest.map_or(score, |lowest| lowest.min(score)));
    }
    lowest
}
