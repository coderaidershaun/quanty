//! The free assessment serves repeat ingestion, preflight and explicit backfill.

use std::path::Path;

use super::{ArtifactRequest, check_budget, checkpoint, generation_signature};
use ocr::content::ChapterArtifacts;

/// Why metadata needs work, or why it cannot currently be completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactState {
    Ready,
    Missing,
    Stale,
    Partial,
    TooLarge,
    NoFolder,
}

impl ArtifactState {
    /// Stable labels printed by the backfill preview.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Partial => "partial",
            Self::TooLarge => "too-large",
            Self::NoFolder => "no-folder",
        }
    }
}

/// No model or store is contacted by this assessment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactAssessment {
    pub state: ArtifactState,
    pub done: usize,
    pub total: usize,
    pub reason: Option<String>,
}

/// Examines saved source and each pending request without paying for a reply.
pub fn assess_artifacts(folder: &Path) -> ArtifactAssessment {
    let chapter = match ocr::read_chapter(folder) {
        Ok(chapter) => chapter,
        Err(error) => {
            return ArtifactAssessment {
                state: ArtifactState::NoFolder,
                done: 0,
                total: 0,
                reason: Some(error.to_string()),
            };
        }
    };
    let source = chapter.artifact_source();
    let current = matches!(ChapterArtifacts::read(folder), Ok(Some(ref file)) if file.matches_source(&source) && file.generation_signature == generation_signature());
    let saved = checkpoint(folder, &source);
    let pending: Vec<_> = source
        .manifest
        .iter()
        .filter(|target| saved.text(&target.id).is_none())
        .cloned()
        .collect();
    let mut result = ArtifactAssessment {
        state: if pending.is_empty() {
            ArtifactState::Ready
        } else if current {
            ArtifactState::Partial
        } else if folder.join(ocr::content::ARTIFACTS_FILE).exists() {
            ArtifactState::Stale
        } else {
            ArtifactState::Missing
        },
        done: saved.entries.len(),
        total: source.manifest.len(),
        reason: None,
    };
    for batch in pending.chunks(20) {
        if let Err(error) = check_budget(
            &ArtifactRequest {
                chapter: &chapter,
                source: &source,
                targets: batch,
                feedback: "",
            }
            .input(),
        ) {
            result.state = ArtifactState::TooLarge;
            result.reason = Some(error.to_string());
            break;
        }
    }
    result
}
