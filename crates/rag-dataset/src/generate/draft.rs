//! The texts the plan asks for, each as a draft that is written and judged on its own, and the
//! record a kept draft becomes.

use std::collections::HashMap;

use rag_core::{ConceptId, Llm};
use serde::Deserialize;

use crate::documents::Split;
use crate::plan::{GenerationPlan, SectionPlan};
use crate::sample::{CptKind, CptSample, Dedup, Gen, PIPELINE_VERSION, Quality, Source, sample_id};
use crate::section::{Provenance, Section, estimated_tokens, passages_of};

use super::asker::{Answer, AskError, Asker};
use super::entigraph::{self, ENTIGRAPH, passage_for};
use super::judge::{self, JUDGE, Verdict};
use super::rewrite::writer_of;
use super::role::Role;

/// What a writer answers: the text it wrote.
#[derive(Debug, Deserialize)]
struct Reply {
    text: String,
}

/// One text to write: what the writer is asked, and the passage the text is judged against.
// SMELL: every draft keeps its own copy of its passage and of its writer's input until the run
// ends, so a run over a whole library holds its text many times over. The drafts of one passage
// could share it, and each input could be made when its draft is written.
pub(super) struct Draft<'a> {
    section: &'a Section,
    concept_ids: &'a [ConceptId],
    kind: CptKind,
    role: &'static Role,
    id: String,
    passage: String,
    input: String,
}

pub(super) enum Outcome {
    /// Boxed because a record is much larger than a rejection.
    Kept(Box<CptSample>),
    /// The writer gave no text, or the judge did not keep it. It counts under its writer's role.
    Rejected { role: &'static Role },
}

/// Every text the plan asks for, in the order of the plan. Nothing is asked here, so the count is
/// known before the first question.
pub(super) fn planned<'a>(plan: &'a GenerationPlan, sections: &'a [Section]) -> Vec<Draft<'a>> {
    let by_id: HashMap<&str, &Section> = sections
        .iter()
        .map(|section| (section.id.as_str(), section))
        .collect();
    let mut drafts = Vec::new();
    for section_plan in &plan.sections {
        let Some(&section) = by_id.get(section_plan.section_id.as_str()) else {
            continue;
        };
        // Every synthetic record is training text, so a held-out section is never rewritten.
        if section.document.split == Split::Train {
            drafts.extend(drafts_of(section_plan, section));
        }
    }
    drafts
}

fn drafts_of<'a>(section_plan: &'a SectionPlan, section: &'a Section) -> Vec<Draft<'a>> {
    let passages = passages_of(section);
    let mut drafts = Vec::new();
    for (&kind, &count) in &section_plan.rewrites {
        // The explanations of relations come from the relations below, one each, whatever count
        // the plan holds for them.
        let Some(role) = writer_of(kind) else {
            continue;
        };
        let mut number = 0;
        for passage in &passages {
            for version in 1..=count {
                number += 1;
                drafts.push(Draft {
                    section,
                    concept_ids: &section_plan.concept_ids,
                    kind,
                    role,
                    id: sample_id(&section.id, kind, Some(number)),
                    passage: passage.clone(),
                    input: writer_input(passage, version),
                });
            }
        }
    }
    for (number, relation) in (1u32..).zip(&section_plan.relations) {
        let passage = passage_for(relation, &passages);
        drafts.push(Draft {
            section,
            concept_ids: &section_plan.concept_ids,
            kind: CptKind::Entigraph,
            role: &ENTIGRAPH,
            id: sample_id(&section.id, CptKind::Entigraph, Some(number)),
            passage: passage.to_owned(),
            input: entigraph::input(relation, passage),
        });
    }
    drafts
}

/// The passage alone for the first version, so that raising a count later reuses the first
/// answer. Later versions end with their number, so that each is a question of its own.
fn writer_input(passage: &str, version: u32) -> String {
    if version == 1 {
        passage.to_owned()
    } else {
        format!("{passage}\n\nVersion {version}")
    }
}

impl Draft<'_> {
    /// Asks the writer for the text and the judge about it. The text is kept when the judge passes
    /// it.
    ///
    /// # Errors
    /// The errors of [`Asker::ask`]: a stop of the model, or a folder of kept answers that cannot
    /// be used. A writer or a judge that gives no answer is a rejected draft, not an error.
    pub(super) async fn write<L: Llm>(&self, asker: &Asker<'_, L>) -> Result<Outcome, AskError> {
        let text = match asker.ask::<Reply>(self.role, &self.input).await? {
            Answer::Given(reply) => reply.text,
            Answer::Failed { reason } => return Ok(self.failed("the writer", &reason)),
        };
        let judged = judge::input(self.kind, &self.passage, &text);
        match asker.ask::<Verdict>(&JUDGE, &judged).await? {
            Answer::Given(verdict) if verdict.passes() => {
                let record = self.record(text, verdict.score, asker.model());
                Ok(Outcome::Kept(Box::new(record)))
            }
            Answer::Given(verdict) => {
                tracing::debug!(
                    "the judge turned down {} with score {}: {:?}",
                    self.id,
                    verdict.score,
                    verdict.unsupported_claims
                );
                Ok(self.rejected())
            }
            Answer::Failed { reason } => Ok(self.failed("the judge", &reason)),
        }
    }

    fn failed(&self, who: &str, reason: &str) -> Outcome {
        tracing::warn!("{who} gave no answer for {}: {reason}", self.id);
        self.rejected()
    }

    fn rejected(&self) -> Outcome {
        Outcome::Rejected { role: self.role }
    }

    fn record(&self, text: String, judge_score: u8, model: &str) -> CptSample {
        let n_tokens = estimated_tokens(&text);
        let sha256 = ocr::sha256_hex(text.as_bytes());
        CptSample {
            id: self.id.clone(),
            kind: self.kind,
            source: Source::of(self.section, self.concept_ids),
            text,
            images: Vec::new(),
            n_tokens,
            tokens_estimated: true,
            provenance: Provenance {
                transcribed_tokens: 0,
                model_written_tokens: n_tokens,
            },
            quality: Quality {
                judge_score: Some(judge_score),
                ..Quality::default()
            },
            dedup: Dedup {
                sha256: Some(sha256),
                minhash_cluster: None,
            },
            r#gen: Gen {
                model: Some(model.to_owned()),
                prompt_id: Some(self.role.prompt_id()),
                faithfulness_checked: Some(true),
            },
            split: Split::Train,
            pipeline_version: PIPELINE_VERSION.to_owned(),
        }
    }
}
