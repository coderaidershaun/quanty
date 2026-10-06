//! A language model that makes up its answers by a rule the test gives, and keeps every question
//! it is asked, so that a test sees what was asked, how often and how many at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rag_core::{Llm, LlmError, Question};
use serde_json::{Value, json};

/// Long enough for the other questions of a batch to start while this one is still open.
const ANSWER_DELAY: Duration = Duration::from_millis(20);

type Rule = dyn Fn(&str, usize) -> Result<Value, LlmError> + Send + Sync;

/// What a question was made of, as owned text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskedQuestion {
    pub system_prompt: String,
    pub schema: String,
    pub input: String,
}

#[derive(Default)]
struct Record {
    questions: Vec<AskedQuestion>,
    asked_before: HashMap<String, usize>,
    open: usize,
    most_open: usize,
}

/// A clone shares what it records with the original, so a test keeps one copy to read after the
/// ingest has used the other.
#[derive(Clone)]
pub struct StandInLlm {
    model: String,
    rule: Arc<Rule>,
    record: Arc<Mutex<Record>>,
}

impl StandInLlm {
    /// Every answer says that the item discusses no concept.
    pub fn finding_nothing() -> StandInLlm {
        StandInLlm::replying(|_, _| Ok(json!({ "concepts": [], "relations": [] })))
    }

    /// `rule(input, earlier_calls)` gives the answer. `earlier_calls` is how many times this exact
    /// input was asked before.
    pub fn replying(
        rule: impl Fn(&str, usize) -> Result<Value, LlmError> + Send + Sync + 'static,
    ) -> StandInLlm {
        StandInLlm {
            model: "stand-in".to_owned(),
            rule: Arc::new(rule),
            record: Arc::new(Mutex::new(Record::default())),
        }
    }

    /// The same stand-in under another model name.
    pub fn named(self, model: &str) -> StandInLlm {
        StandInLlm {
            model: model.to_owned(),
            ..self
        }
    }

    pub fn questions(&self) -> Vec<AskedQuestion> {
        self.record.lock().unwrap().questions.clone()
    }

    pub fn calls(&self) -> usize {
        self.record.lock().unwrap().questions.len()
    }

    pub fn most_calls_at_once(&self) -> usize {
        self.record.lock().unwrap().most_open
    }
}

impl Llm for StandInLlm {
    fn model(&self) -> &str {
        &self.model
    }

    async fn ask(&self, question: Question<'_>) -> Result<Value, LlmError> {
        let earlier_calls = {
            let mut record = self.record.lock().unwrap();
            record.questions.push(AskedQuestion {
                system_prompt: question.system_prompt.to_owned(),
                schema: question.schema.to_owned(),
                input: question.input.to_owned(),
            });
            record.open += 1;
            record.most_open = record.most_open.max(record.open);
            let earlier = record
                .asked_before
                .entry(question.input.to_owned())
                .or_insert(0);
            let before = *earlier;
            *earlier += 1;
            before
        };
        tokio::time::sleep(ANSWER_DELAY).await;
        self.record.lock().unwrap().open -= 1;
        (self.rule)(question.input, earlier_calls)
    }
}
