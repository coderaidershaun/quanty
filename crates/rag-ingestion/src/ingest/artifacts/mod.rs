//! Required display metadata is checkpointed before searchable data can be replaced.

mod assessment;
mod question;

use std::path::Path;

use ocr::content::{
    ARTIFACTS_VERSION, ArtifactEntry, ArtifactSource, ArtifactTarget, ChapterArtifacts,
    ContentError,
};
use rag_core::{Llm, LlmError, Question, UsageTally};

use super::{IngestStep, Meter, TRIES_FOR_ONE_QUESTION};
pub use assessment::{ArtifactAssessment, ArtifactState, assess_artifacts};
use question::{ArtifactRequest, MAX_BYTES, PROMPT, SCHEMA, validate};

/// Counts completed display entries and what this run paid for.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactSummary {
    pub total: usize,
    pub written: usize,
    pub usage: UsageTally,
}

/// Required metadata failures leave accepted batches available to the next run.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("could not read converted source for artifact metadata")]
    Read(#[from] ocr::ReadChapterError),
    #[error("could not save artifact metadata")]
    Save(#[from] ContentError),
    #[error(
        "artifact metadata request is {bytes} UTF-8 bytes, above the conservative local {MAX_BYTES}-byte budget; keep the chapter and resume after changing the request policy"
    )]
    TooLarge { bytes: usize },
    #[error("artifact metadata failed after two attempts: {diagnostics}")]
    Rejected { diagnostics: String },
    #[error("the artifact metadata model could not answer")]
    Model(#[source] LlmError),
}

impl ArtifactError {
    /// A systemic model failure stops a bulk command immediately.
    pub fn stops_the_run(&self) -> bool {
        matches!(self, Self::Model(error) if error.stops_the_run())
    }
}

/// Uses the selected Ingest model, retaining accepted work across model changes.
pub struct ArtifactWriter<L> {
    llm: L,
}

impl<L: Llm> ArtifactWriter<L> {
    pub fn new(llm: L) -> Self {
        Self { llm }
    }

    /// Writes only missing batches, retaining each wholly valid batch atomically.
    ///
    /// # Errors
    /// Returns source, persistence, request budget, model or validation failures.
    pub async fn write_missing(&self, folder: &Path) -> Result<ArtifactSummary, ArtifactError> {
        self.write_missing_reporting(
            folder,
            &mut Meter::new(&mut |_, _| {}, UsageTally::default()),
        )
        .await
    }

    /// Reports cumulative usage after every returned reply, including rejected replies.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::write_missing`]. Accepted batches remain saved.
    pub async fn write_missing_with_progress(
        &self,
        folder: &Path,
        on_step: &mut (dyn FnMut(IngestStep, &UsageTally) + Send),
    ) -> Result<ArtifactSummary, ArtifactError> {
        self.write_missing_reporting(folder, &mut Meter::new(on_step, UsageTally::default()))
            .await
    }

    pub(crate) async fn write_missing_reporting(
        &self,
        folder: &Path,
        meter: &mut Meter<'_>,
    ) -> Result<ArtifactSummary, ArtifactError> {
        let chapter = ocr::read_chapter(folder)?;
        let source = chapter.artifact_source();
        let mut saved = checkpoint(folder, &source);
        let pending = pending(&source, &saved);
        let total = source.manifest.len();
        let mut usage = UsageTally::default();
        let mut written = 0;
        for targets in pending.chunks(BATCH_SIZE) {
            let mut feedback = String::new();
            let mut accepted = None;
            for _ in 0..TRIES_FOR_ONE_QUESTION {
                let input = ArtifactRequest {
                    chapter: &chapter,
                    source: &source,
                    targets,
                    feedback: &feedback,
                }
                .input();
                check_budget(&input)?;
                meter.step(IngestStep::WritingArtifacts {
                    done: saved.entries.len(),
                    total,
                });
                let reply = match self
                    .llm
                    .ask_long(Question {
                        system_prompt: PROMPT,
                        schema: SCHEMA,
                        input: &input,
                    })
                    .await
                {
                    Ok(reply) => reply,
                    Err(error) if error.stops_the_run() => return Err(ArtifactError::Model(error)),
                    Err(error) => {
                        feedback = error.to_string();
                        continue;
                    }
                };
                usage.add_all(&reply.usage);
                meter.spend(&reply.usage);
                meter.step(IngestStep::WritingArtifacts {
                    done: saved.entries.len(),
                    total,
                });
                match validate(reply.value, targets) {
                    Ok(texts) => {
                        accepted = Some(texts);
                        break;
                    }
                    Err(diagnostics) => feedback = diagnostics.to_string(),
                }
            }
            let texts = accepted.ok_or(ArtifactError::Rejected {
                diagnostics: feedback,
            })?;
            written += texts.len();
            saved
                .entries
                .extend(texts.into_iter().map(|text| ArtifactEntry {
                    text,
                    model: self.llm.model().to_owned(),
                    cache_identity: self.llm.cache_identity(),
                }));
            saved.write(folder)?;
            meter.step(IngestStep::WritingArtifacts {
                done: saved.entries.len(),
                total,
            });
        }
        if pending.is_empty()
            && !matches!(ChapterArtifacts::read(folder), Ok(Some(ref file)) if file == &saved)
        {
            saved.write(folder)?;
        }
        Ok(ArtifactSummary {
            total,
            written,
            usage,
        })
    }
}

/// How many targets one question asks for.
const BATCH_SIZE: usize = 20;

/// The targets that have no accepted text yet.
fn pending(source: &ArtifactSource, saved: &ChapterArtifacts) -> Vec<ArtifactTarget> {
    source
        .manifest
        .iter()
        .filter(|target| saved.text(&target.id).is_none())
        .cloned()
        .collect()
}

// Raise this when chapter rendering or acceptance rules change, so old work is regenerated.
const GENERATION_POLICY_VERSION: &str = "2";

fn generation_signature() -> String {
    ocr::sha256_hex(format!("{GENERATION_POLICY_VERSION}:{PROMPT}:{SCHEMA}").as_bytes())
}

fn checkpoint(folder: &Path, source: &ocr::content::ArtifactSource) -> ChapterArtifacts {
    let signature = generation_signature();
    if let Ok(Some(saved)) = ChapterArtifacts::read(folder)
        && saved.matches_source(source)
        && saved.generation_signature == signature
    {
        return saved;
    }
    ChapterArtifacts {
        format_version: ARTIFACTS_VERSION,
        source: source.clone(),
        generation_signature: signature,
        entries: Vec::new(),
    }
}

fn check_budget(input: &str) -> Result<(), ArtifactError> {
    let bytes = PROMPT.len() + SCHEMA.len() + input.len();
    if bytes > MAX_BYTES {
        return Err(ArtifactError::TooLarge { bytes });
    }
    Ok(())
}
