//! The first half of extraction: asks about every item, a few at a time, and keeps each good
//! answer. One failed item never stops the others; a failure all items share stops the run.

use std::error::Error;

use futures_util::StreamExt;
use futures_util::stream;
use rag_core::{Embedding, ItemFilter, ItemId, ItemStore, Llm, LlmError, Question};
use serde_json::Value;

use super::cache::{Cache, key_of};
use super::question::{Extraction, SCHEMA, SYSTEM_PROMPT, input_for, related_material};
use super::{ConceptError, ConceptExtractor, EmbeddedItems, SkippedItem};
use crate::ingest::items::Item;
use crate::ingest::{IngestStep, OnStep};

const CALLS_AT_A_TIME: usize = 4;
/// How many of the stored items nearest to a lone item are added to its question.
const RELATED_ITEMS_KEPT: usize = 5;
/// One more is looked at than is kept, because one of them is the item itself, which is already
/// stored.
const RELATED_ITEMS_SEARCHED: usize = RELATED_ITEMS_KEPT + 1;
/// A question that fails is asked once more before it counts as failed.
const TRIES_FOR_ONE_QUESTION: usize = 2;

/// What the first half found, with the items in the order they came in.
#[derive(Default)]
pub(super) struct Answers {
    pub extractions: Vec<ItemAnswer>,
    pub llm_calls: usize,
    pub cache_hits: usize,
    pub skipped: Vec<SkippedItem>,
}

pub(super) struct ItemAnswer {
    pub item: ItemId,
    /// How many items of the run come before this one, the skipped ones among them.
    pub items_before: usize,
    pub extraction: Extraction,
}

pub(super) struct Reading<T> {
    pub llm_calls: usize,
    pub outcome: Outcome<T>,
}

pub(super) enum Outcome<T> {
    Cached(T),
    Asked(T),
    /// Both tries failed, and this is why the last one did.
    Failed(String),
    Stopped(LlmError),
}

pub(super) struct KeyedQuestion<'a> {
    pub key: String,
    pub question: Question<'a>,
}

impl<L: Llm> ConceptExtractor<L> {
    /// The answers come back in item order, so a stop is seen only after every item before it has
    /// been answered and kept.
    ///
    /// An item that is alone in its document is asked about together with the nearest stored items
    /// of other documents, so that the model names its concepts as they are named elsewhere.
    ///
    /// # Errors
    /// - [`ConceptError::Stopped`] at the first item whose question failed in a way that would
    ///   fail every other question too. The items after it are not asked.
    /// - [`ConceptError::Cache`] when the cache folder cannot be used
    /// - [`ConceptError::RelatedItems`] when the stored items cannot be searched
    pub(super) async fn read_items(
        &self,
        cache: &Cache,
        embedded: &EmbeddedItems<'_>,
        stored_items: &ItemStore,
        on_step: OnStep<'_>,
    ) -> Result<Answers, ConceptError> {
        let items = embedded.items;
        let related = match (items, embedded.vectors) {
            ([item], [vector]) => related_to(item, vector.clone(), stored_items).await?,
            _ => String::new(),
        };
        let related = related.as_str();
        // Every read is made here, before the stream, and none starts until the stream polls it.
        // Do not make them with a closure inside the stream: the compiler then cannot prove that
        // this future is `Send`, and no caller could spawn an ingest.
        let item_reads: Vec<_> = items
            .iter()
            .map(|item| async move { (item, self.read_one(cache, item, related).await) })
            .collect();
        let mut reads = stream::iter(item_reads).buffered(CALLS_AT_A_TIME);
        let mut answers = Answers::default();
        let mut taken = 0;
        // One result is taken at a time, because after a stop no new item may be asked. Returning
        // drops the reads that are left.
        while let Some((item, read)) = reads.next().await {
            let read = read?;
            answers.llm_calls += read.llm_calls;
            let answered = |extraction| ItemAnswer {
                item: item.id,
                items_before: taken,
                extraction,
            };
            match read.outcome {
                Outcome::Cached(extraction) => {
                    answers.cache_hits += 1;
                    answers.extractions.push(answered(extraction));
                }
                Outcome::Asked(extraction) => answers.extractions.push(answered(extraction)),
                Outcome::Failed(reason) => answers.skipped.push(SkippedItem {
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
            on_step(IngestStep::ReadingConcepts {
                done: taken,
                total: items.len(),
            });
        }
        Ok(answers)
    }

    /// `related` is added to the message after the key is made, so it never changes the key.
    async fn read_one(
        &self,
        cache: &Cache,
        item: &Item,
        related: &str,
    ) -> Result<Reading<Extraction>, ConceptError> {
        let input = input_for(item);
        let key = key_of(
            &self.prompt_version,
            self.llm.model(),
            Question {
                system_prompt: SYSTEM_PROMPT,
                schema: SCHEMA,
                input: &input,
            },
        );
        let message = format!("{input}{related}");
        let asked = KeyedQuestion {
            key,
            question: Question {
                system_prompt: SYSTEM_PROMPT,
                schema: SCHEMA,
                input: &message,
            },
        };
        self.answer(cache, &asked, Extraction::from_value).await
    }

    /// The answer kept under the key, or else the model's answer to the question, asked up to
    /// twice. `read` turns a reply into the answer, and a reply it refuses counts as a failed try.
    /// A good reply is kept, so the question is not asked again.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the cache folder cannot be used.
    pub(super) async fn answer<T, E: Error>(
        &self,
        cache: &Cache,
        asked: &KeyedQuestion<'_>,
        read: impl Fn(&Value) -> Result<T, E>,
    ) -> Result<Reading<T>, ConceptError> {
        let key = asked.key.as_str();
        let question = asked.question;
        if let Some(kept) = cache.get(key)?
            && let Ok(answer) = read(&kept)
        {
            return Ok(Reading {
                llm_calls: 0,
                outcome: Outcome::Cached(answer),
            });
        }
        let mut llm_calls = 0;
        let mut reason = String::new();
        for _ in 0..TRIES_FOR_ONE_QUESTION {
            llm_calls += 1;
            let reply = match self.llm.ask(question).await {
                Ok(reply) => reply,
                Err(error) if error.stops_the_run() => {
                    return Ok(Reading {
                        llm_calls,
                        outcome: Outcome::Stopped(error),
                    });
                }
                Err(error) => {
                    reason = chain_of(&error);
                    continue;
                }
            };
            match read(&reply) {
                Ok(answer) => {
                    cache.put(key, &reply)?;
                    return Ok(Reading {
                        llm_calls,
                        outcome: Outcome::Asked(answer),
                    });
                }
                Err(error) => reason = chain_of(&error),
            }
        }
        Ok(Reading {
            llm_calls,
            outcome: Outcome::Failed(reason),
        })
    }
}

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

/// The related material for an item that is alone in its document: the passages of the nearest
/// stored items that belong to other documents. It is empty when there are none.
async fn related_to(
    item: &Item,
    vector: Embedding,
    stored_items: &ItemStore,
) -> Result<String, ConceptError> {
    let hits = stored_items
        .search(vector, &ItemFilter::default(), RELATED_ITEMS_SEARCHED)
        .await
        .map_err(|source| ConceptError::RelatedItems {
            item: item.id,
            source,
        })?;
    // The item is stored before it is asked about, so its own point is among the nearest.
    let others: Vec<_> = hits
        .into_iter()
        .filter(|hit| hit.payload.doc_id != item.payload.doc_id)
        .take(RELATED_ITEMS_KEPT)
        .collect();
    Ok(related_material(&others))
}
