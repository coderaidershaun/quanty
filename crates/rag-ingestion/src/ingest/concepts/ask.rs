//! The first half of extraction: asks about every item, a few at a time, and keeps each good
//! answer. A failure of one item never stops the others. A failure that would be the same for
//! every item stops the run.

use std::error::Error;

use futures_util::StreamExt;
use futures_util::stream;
use rag_core::{ItemId, Llm, LlmError, Question};

use super::cache::{Cache, key_of};
use super::question::{Extraction, SCHEMA, SYSTEM_PROMPT, input_for};
use super::{ConceptError, ConceptExtractor, SkippedItem};
use crate::ingest::items::Item;

const CALLS_AT_A_TIME: usize = 4;
/// A question that fails is asked once more before its item is skipped.
const TRIES_FOR_ONE_ITEM: usize = 2;

/// What the first half found, with the items in the order they came in.
#[derive(Default)]
pub(super) struct Answers {
    pub extractions: Vec<(ItemId, Extraction)>,
    pub llm_calls: usize,
    pub cache_hits: usize,
    pub skipped: Vec<SkippedItem>,
}

struct ItemRead {
    llm_calls: usize,
    outcome: Outcome,
}

enum Outcome {
    Cached(Extraction),
    Asked(Extraction),
    Skipped(String),
    Stopped(LlmError),
}

impl<L: Llm> ConceptExtractor<L> {
    /// Asks about every item and returns what was found. The answers come back in item order, so
    /// a stop is seen only after every item before it has been answered and kept.
    ///
    /// # Errors
    /// - [`ConceptError::Stopped`] at the first item whose question failed in a way that would
    ///   fail every other question too. The items after it are not asked.
    /// - [`ConceptError::Cache`] when the cache folder cannot be used
    pub(super) async fn read_items(&self, items: &[Item]) -> Result<Answers, ConceptError> {
        let cache = &Cache::open(&self.cache_folder)?;
        // Every read is made here, before the stream, and none starts until the stream polls it.
        // Do not make them with a closure inside the stream: the compiler then cannot prove that
        // this future is `Send`, and no caller could spawn an ingest.
        let item_reads: Vec<_> = items
            .iter()
            .map(|item| async move { (item, self.read_one(cache, item).await) })
            .collect();
        let mut reads = stream::iter(item_reads).buffered(CALLS_AT_A_TIME);
        let mut answers = Answers::default();
        let mut taken = 0;
        // One result is taken at a time, because after a stop no new item may be asked. Returning
        // drops the reads that are left.
        while let Some((item, read)) = reads.next().await {
            let read = read?;
            answers.llm_calls += read.llm_calls;
            match read.outcome {
                Outcome::Cached(extraction) => {
                    answers.cache_hits += 1;
                    answers.extractions.push((item.id, extraction));
                }
                Outcome::Asked(extraction) => answers.extractions.push((item.id, extraction)),
                Outcome::Skipped(reason) => answers.skipped.push(SkippedItem {
                    id: item.id,
                    kind: item.payload.kind,
                    page: item.payload.page,
                    reason,
                }),
                Outcome::Stopped(source) => {
                    return Err(ConceptError::Stopped {
                        read: taken,
                        items: items.len(),
                        source,
                    });
                }
            }
            taken += 1;
        }
        Ok(answers)
    }

    async fn read_one(&self, cache: &Cache, item: &Item) -> Result<ItemRead, ConceptError> {
        let input = input_for(item);
        let question = Question {
            system_prompt: SYSTEM_PROMPT,
            schema: SCHEMA,
            input: &input,
        };
        let key = key_of(&self.prompt_version, self.llm.model(), question);
        if let Some(extraction) = cache.get(&key)? {
            return Ok(ItemRead {
                llm_calls: 0,
                outcome: Outcome::Cached(extraction),
            });
        }
        let mut llm_calls = 0;
        let mut reason = String::new();
        for _ in 0..TRIES_FOR_ONE_ITEM {
            llm_calls += 1;
            let answer = match self.llm.ask(question).await {
                Ok(answer) => answer,
                Err(error) if error.stops_the_run() => {
                    return Ok(ItemRead {
                        llm_calls,
                        outcome: Outcome::Stopped(error),
                    });
                }
                Err(error) => {
                    reason = chain_of(&error);
                    continue;
                }
            };
            match Extraction::from_value(&answer) {
                Ok(extraction) => {
                    cache.put(&key, &answer)?;
                    return Ok(ItemRead {
                        llm_calls,
                        outcome: Outcome::Asked(extraction),
                    });
                }
                Err(error) => reason = chain_of(&error),
            }
        }
        Ok(ItemRead {
            llm_calls,
            outcome: Outcome::Skipped(reason),
        })
    }
}

/// The error and what caused it, in one line, so that a summary says all of why an item failed.
fn chain_of(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
