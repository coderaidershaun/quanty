//! The folder of kept answers: one plain file for each question that was answered well, so that
//! the same question is never paid for twice.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rag_core::{Llm, LlmError, Question, UsageTally, error_chain};
use serde_json::Value;

use super::ConceptError;

const FILE_EXTENSION: &str = "json";
/// How many questions the programs ask at one time.
pub const CALLS_AT_A_TIME: usize = 4;
/// A question that fails is asked once more before it counts as failed.
pub const TRIES_FOR_ONE_QUESTION: usize = 2;

pub struct Reading<T> {
    pub llm_calls: usize,
    /// What the replies used, counting a reply that was refused, because it was paid for.
    pub usage: UsageTally,
    pub outcome: Outcome<T>,
}

pub enum Outcome<T> {
    Cached(T),
    Asked(T),
    /// Both tries failed, and this is why the last one did.
    Failed(String),
    Stopped(LlmError),
}

/// The folder of kept answers, shared by every program that asks the model the same kind of
/// question.
pub struct Cache {
    folder: PathBuf,
}

impl Cache {
    /// The name that an answer is kept under. It is made from the prompt version, the model, the
    /// prompt, the schema and the input, so a person who edits the prompt or the schema and
    /// forgets to raise the version still gets fresh answers. The caller leaves out of the input
    /// what only helps the asking, such as the related material of an item that is alone in its
    /// document. With that in the name, the same item would be asked about again each time the
    /// stores had changed.
    pub fn key_of(prompt_version: &str, model: &str, question: Question<'_>) -> String {
        let mut hashed = Vec::new();
        for part in [
            prompt_version,
            model,
            question.system_prompt,
            question.schema,
            question.input,
        ] {
            // The length goes first, so that the end of one part cannot be taken for the start of
            // the next.
            hashed.extend(part.len().to_le_bytes());
            hashed.extend(part.as_bytes());
        }
        ocr::sha256_hex(&hashed)
    }

    /// Makes the folder when it is missing.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the folder cannot be made.
    pub fn open(folder: &Path) -> Result<Cache, ConceptError> {
        fs::create_dir_all(folder).map_err(|source| ConceptError::Cache {
            path: folder.to_path_buf(),
            source,
        })?;
        Ok(Cache {
            folder: folder.to_path_buf(),
        })
    }

    fn path_of(&self, key: &str) -> PathBuf {
        self.folder.join(key).with_extension(FILE_EXTENSION)
    }

    /// The kept answer, or `None` when there is none. A file that is not JSON counts as no answer:
    /// the next good answer overwrites it. The caller checks that the answer fits the reply it
    /// expects, and counts one that does not fit as no answer too.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the file exists but cannot be read.
    pub fn get(&self, key: &str) -> Result<Option<Value>, ConceptError> {
        let path = self.path_of(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(ConceptError::Cache { path, source }),
        };
        Ok(serde_json::from_slice(&bytes).ok())
    }

    /// Keeps the answer exactly as the model gave it. It is written under a temporary name and
    /// then renamed, so a run that is killed leaves no half-written file.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the file cannot be written.
    pub fn put(&self, key: &str, answer: &Value) -> Result<(), ConceptError> {
        let path = self.path_of(key);
        let temporary = self
            .folder
            .join(format!("{key}.{}.tmp", std::process::id()));
        let written = serde_json::to_vec(answer)
            .map_err(io::Error::other)
            .and_then(|bytes| fs::write(&temporary, bytes))
            .and_then(|()| fs::rename(&temporary, &path));
        written.map_err(|source| ConceptError::Cache { path, source })
    }

    /// The answer kept under the key, or else the model's answer to the question, asked up to
    /// twice. `read` turns a reply into the answer, and a reply it refuses counts as a failed try.
    /// A good reply is kept, so the question is not asked again.
    ///
    /// # Errors
    /// [`ConceptError::Cache`] when the cache folder cannot be used.
    pub async fn answer<T, E: Error>(
        &self,
        llm: &impl Llm,
        key: &str,
        question: Question<'_>,
        read: impl Fn(&Value) -> Result<T, E>,
    ) -> Result<Reading<T>, ConceptError> {
        if let Some(kept) = self.get(key)?
            && let Ok(answer) = read(&kept)
        {
            return Ok(Reading {
                llm_calls: 0,
                usage: UsageTally::default(),
                outcome: Outcome::Cached(answer),
            });
        }
        let mut llm_calls = 0;
        let mut usage = UsageTally::default();
        let mut reason = String::new();
        for _ in 0..TRIES_FOR_ONE_QUESTION {
            llm_calls += 1;
            let reply = match llm.ask(question).await {
                Ok(reply) => reply,
                Err(error) if error.stops_the_run() => {
                    let outcome = Outcome::Stopped(error);
                    return Ok(Reading {
                        llm_calls,
                        usage,
                        outcome,
                    });
                }
                Err(error) => {
                    reason = error_chain(&error);
                    continue;
                }
            };
            usage.add_all(&reply.usage);
            match read(&reply.value) {
                Ok(answer) => {
                    self.put(key, &reply.value)?;
                    let outcome = Outcome::Asked(answer);
                    return Ok(Reading {
                        llm_calls,
                        usage,
                        outcome,
                    });
                }
                Err(error) => reason = error_chain(&error),
            }
        }
        Ok(Reading {
            llm_calls,
            usage,
            outcome: Outcome::Failed(reason),
        })
    }
}
