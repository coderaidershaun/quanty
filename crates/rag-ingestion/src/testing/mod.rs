//! Stand-ins, throwaway stores and converted chapters to ingest, for the tests of this crate and of
//! the crates that read what it stores. They are behind the cargo feature `testing` and panic with
//! a clear message on failure.

mod stand_in_embedder;
mod stand_in_llm;
mod throwaway_stores;

use std::collections::BTreeSet;
use std::path::Path;

use rag_core::{Category, MediaLabels};

use crate::ChapterFolder;
pub use stand_in_embedder::{StandInEmbedder, first_axis, vector_at};
pub use stand_in_llm::{AskedQuestion, StandInLlm};
pub use throwaway_stores::ThrowawayStores;

/// The converted chapter in `folder`. Its media, when the graph does not have it yet, is made
/// with no labels.
pub fn chapter_at(folder: &Path) -> ChapterFolder<'_> {
    static NO_LABELS: MediaLabels = MediaLabels {
        category: Category::Book,
        authors: Vec::new(),
        tags: BTreeSet::new(),
    };
    ChapterFolder {
        folder,
        new_media: &NO_LABELS,
    }
}
