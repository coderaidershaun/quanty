//! Asks the model for the datasets and keeps what it said: an answer already given is read from
//! the folder of kept answers, a failed question is asked once more, and a failure that would fail
//! every question stops the run.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use futures_util::Stream;
use futures_util::StreamExt;
use futures_util::stream;
use rag_core::{Llm, LlmError, UsageTally, error_chain};
use rag_ingestion::{AnswerCache, ConceptError};
use serde::de::DeserializeOwned;

use super::role::Role;

const CALLS_AT_A_TIME: usize = 4;
/// A question that fails is asked once more before it counts as failed.
const TRIES_FOR_ONE_QUESTION: usize = 2;

/// What the model was asked so far and what it cost.
#[derive(Default)]
struct Record {
    usage: UsageTally,
    calls: usize,
    cache_hits: usize,
}

/// A language model, the folder where its answers are kept, and a count of what it was asked.
pub struct Asker<'a, L> {
    llm: &'a L,
    cache: AnswerCache,
    record: Mutex<Record>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Answer<T> {
    Given(T),
    /// Both tries failed, and this is why the last one did.
    Failed {
        reason: String,
    },
}

#[derive(thiserror::Error, Debug)]
pub enum AskError {
    #[error("the model can answer no question now")]
    Stopped(#[source] LlmError),

    #[error(
        "could not use the folder of kept answers; check that DATASET_CACHE_DIR names a folder that can be read and written"
    )]
    Cache(#[from] ConceptError),
}

impl<'a, L: Llm> Asker<'a, L> {
    /// Makes the cache folder when it is missing.
    ///
    /// # Errors
    /// [`AskError::Cache`] when the folder cannot be made.
    pub fn new(llm: &'a L, cache_folder: &Path) -> Result<Self, AskError> {
        Ok(Asker {
            llm,
            cache: AnswerCache::open(cache_folder)?,
            record: Mutex::new(Record::default()),
        })
    }

    pub fn model(&self) -> &str {
        self.llm.model()
    }

    /// The answer kept for this question, or else the model's answer, asked up to twice. A reply
    /// that does not fit `T` counts as a failed try. A good reply is kept, so the question is not
    /// asked again.
    ///
    /// # Errors
    /// - [`AskError::Stopped`] when the question failed in a way that would fail every other
    ///   question too, such as a usage limit or a model that is signed out. It is not asked again.
    /// - [`AskError::Cache`] when the folder of kept answers cannot be used
    pub async fn ask<T: DeserializeOwned + Send>(
        &self,
        role: &Role,
        input: &str,
    ) -> Result<Answer<T>, AskError> {
        let question = role.question(input);
        let key = AnswerCache::key_of(&role.prompt_id(), &self.llm.cache_identity(), question);
        if let Some(kept) = self.cache.get(&key)?
            && let Ok(answer) = serde_json::from_value::<T>(kept)
        {
            self.record().cache_hits += 1;
            return Ok(Answer::Given(answer));
        }
        let mut reason = String::new();
        for _ in 0..TRIES_FOR_ONE_QUESTION {
            self.record().calls += 1;
            let reply = match self.llm.ask(question).await {
                Ok(reply) => reply,
                Err(error) if error.stops_the_run() => return Err(AskError::Stopped(error)),
                Err(error) => {
                    reason = error_chain(&error);
                    continue;
                }
            };
            // A reply that does not fit was paid for too.
            self.record().usage.add_all(&reply.usage);
            match serde_json::from_value::<T>(reply.value.clone()) {
                Ok(answer) => {
                    self.cache.put(&key, &reply.value)?;
                    return Ok(Answer::Given(answer));
                }
                Err(error) => reason = error_chain(&error),
            }
        }
        Ok(Answer::Failed { reason })
    }

    /// Asks every input, a few at a time. The answers come back in the order of the inputs.
    ///
    /// # Errors
    /// The errors of [`Asker::ask`]. The asks still running when one fails are dropped, and the
    /// inputs after them are not asked.
    pub async fn ask_each<T: DeserializeOwned + Send>(
        &self,
        role: &Role,
        inputs: &[String],
    ) -> Result<Vec<Answer<T>>, AskError> {
        // Every ask is made here, before the stream, and none starts until the stream polls it.
        // Do not make them with a closure inside the stream: the compiler then cannot prove that
        // this future is `Send`, and no caller could spawn a build.
        let asks: Vec<_> = inputs
            .iter()
            .map(|input| self.ask::<T>(role, input))
            .collect();
        let mut asked = stream::iter(asks).buffered(CALLS_AT_A_TIME);
        let mut answers = Vec::with_capacity(inputs.len());
        while let Some(answer) = asked.next().await {
            answers.push(answer?);
        }
        Ok(answers)
    }

    /// What every reply of this asker used. An answer read from the kept answers used nothing.
    pub fn usage(&self) -> UsageTally {
        self.record().usage.clone()
    }

    /// The questions sent to the model, second tries included.
    pub fn calls(&self) -> usize {
        self.record().calls
    }

    /// The questions that a kept answer settled.
    pub fn cache_hits(&self) -> usize {
        self.record().cache_hits
    }

    /// The counts are only counts, so a lock that a panic poisoned is still good to read.
    fn record(&self) -> MutexGuard<'_, Record> {
        self.record.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs the asks a few at a time, as many at once as [`Asker::ask_each`] asks, and gives their
/// results in the order of the asks. Make every ask before the call, not with a closure inside
/// the stream: the compiler then cannot prove that the caller's future is `Send`.
pub(super) fn few_at_a_time<F: Future>(asks: Vec<F>) -> impl Stream<Item = F::Output> {
    stream::iter(asks).buffered(CALLS_AT_A_TIME)
}
