//! The first half of extraction: asks about every item, a few at a time, and keeps each good
//! answer. One failed item never stops the others; a failure all items share stops the run.

use futures_util::StreamExt;
use futures_util::stream;
use rag_core::{Embedding, ItemFilter, ItemId, ItemStore, Llm, Question};

use super::cache::{CALLS_AT_A_TIME, Cache, Outcome, Reading};
use super::question::{Extraction, SCHEMA, SYSTEM_PROMPT, input_for, related_material};
use super::{ConceptError, ConceptExtractor, EmbeddedItems, SkippedItem};
use crate::ingest::items::Item;
use crate::ingest::{IngestStep, Meter};

/// How many of the stored items nearest to a lone item are added to its question.
const RELATED_ITEMS_KEPT: usize = 5;
/// One more is looked at than is kept, because one of them is the item itself, which is already
/// stored.
const RELATED_ITEMS_SEARCHED: usize = RELATED_ITEMS_KEPT + 1;

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
        meter: &mut Meter<'_>,
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
            meter.spend(&read.usage);
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
            meter.step(IngestStep::ReadingConcepts {
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
        let key = Cache::key_of(
            &self.prompt_version,
            &self.llm.cache_identity(),
            Question {
                system_prompt: SYSTEM_PROMPT,
                schema: SCHEMA,
                input: &input,
            },
        );
        let message = format!("{input}{related}");
        let question = Question {
            system_prompt: SYSTEM_PROMPT,
            schema: SCHEMA,
            input: &message,
        };
        cache
            .answer(&self.llm, &key, question, Extraction::from_value)
            .await
    }
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
