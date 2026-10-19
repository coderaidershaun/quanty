//! The records of the fine-tuning files, made from the questions and what the checks found, and
//! the count of what was written.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::documents::Split;
use crate::progress::Written;
use crate::sample::{Message, PIPELINE_VERSION, Seed, SftSample, Speaker, sft_id};

use super::questions::{Asked, json_name};
use super::solve::reasoning_of;
use super::verify::{Failure, Verified};

/// A record that the checks kept out, with the reason, as the continued pre-training files do it.
#[derive(Serialize)]
pub(super) struct RejectedSft {
    #[serde(flatten)]
    sample: SftSample,
    /// The name of the failure, such as `disagreement`.
    rejected: String,
}

/// What the stage has made so far, and the count of it.
#[derive(Default)]
pub(super) struct Records {
    pub(super) train: Vec<SftSample>,
    pub(super) heldout: Vec<SftSample>,
    pub(super) rejected: Vec<RejectedSft>,
    pub(super) written: Written,
}

impl Records {
    /// A question that passed goes to the file of its side of the held-out line. One that failed
    /// goes to the rejected file, whichever side it is on.
    pub(super) fn add(&mut self, asked: &Asked<'_>, verified: Verified) {
        let failure = verified.failure;
        let sample = record(asked, verified);
        match failure {
            None => {
                add_count(&mut self.written.by_kind, &json_name(sample.task), 1);
                add_count(
                    &mut self.written.by_reasoning,
                    &json_name(sample.reasoning),
                    1,
                );
                match sample.split {
                    Split::Train => self.train.push(sample),
                    Split::Heldout => self.heldout.push(sample),
                }
            }
            Some(failure) => {
                add_count(&mut self.written.rejected, failure.name(), 1);
                self.rejected.push(RejectedSft {
                    sample,
                    rejected: failure.name().to_owned(),
                });
            }
        }
    }

    /// The planned questions of sections for which the model wrote none are counted, and have no
    /// record, because there is no question to put in it.
    pub(super) fn add_unanswered(&mut self, count: usize) {
        if count > 0 {
            add_count(&mut self.written.rejected, Failure::NoAnswer.name(), count);
        }
    }
}

fn add_count(counts: &mut BTreeMap<String, usize>, name: &str, count: usize) {
    *counts.entry(name.to_owned()).or_insert(0) += count;
}

fn record(asked: &Asked<'_>, verified: Verified) -> SftSample {
    let written = &asked.written;
    let verification = verified.verification();
    let mut messages = vec![Message {
        role: Speaker::User,
        thinking: None,
        content: written.question.clone(),
    }];
    if let Some(kept) = verified.kept {
        messages.push(Message {
            role: Speaker::Assistant,
            thinking: kept.thinking,
            content: kept.answer,
        });
    }
    SftSample {
        id: sft_id(&asked.section.id, asked.number),
        task: written.task,
        reasoning: reasoning_of(written.task),
        messages,
        seed: Seed {
            kind: written.seed.kind,
            item_ids: asked.section.item_ids.clone(),
            section_path: asked.section.section_path.clone(),
            quote: written.seed.quote.clone(),
        },
        verification,
        known_by_cpt_model: None,
        difficulty: written.difficulty.clamp(0.0, 1.0),
        ifd: None,
        split: asked.section.document.split,
        pipeline_version: PIPELINE_VERSION.to_owned(),
    }
}
