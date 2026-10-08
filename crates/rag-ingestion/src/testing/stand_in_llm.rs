//! A language model that makes up its answers by a rule the test gives, and keeps every question
//! it is asked, so that a test sees what was asked, how often and how many at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rag_core::{Llm, LlmError, Question};
use serde_json::{Value, json};

/// Long enough for the other questions of a batch to start while this one is still open.
const ANSWER_DELAY: Duration = Duration::from_millis(20);

type Rule = dyn Fn(&AskedQuestion, usize) -> Result<Value, LlmError> + Send + Sync;

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
    pub fn finding_nothing() -> StandInLlm {
        StandInLlm::replying(|_, _| Ok(json!({ "concepts": [], "relations": [] })))
    }

    /// `rule(input, earlier_calls)` gives the answer. `earlier_calls` is how many times this exact
    /// input was asked before.
    pub fn replying(
        rule: impl Fn(&str, usize) -> Result<Value, LlmError> + Send + Sync + 'static,
    ) -> StandInLlm {
        StandInLlm::replying_to_questions(move |question, earlier_calls| {
            rule(&question.input, earlier_calls)
        })
    }

    /// Like [`StandInLlm::replying`], and the rule sees the whole question, so it can tell one kind
    /// of question from another by its prompt.
    pub fn replying_to_questions(
        rule: impl Fn(&AskedQuestion, usize) -> Result<Value, LlmError> + Send + Sync + 'static,
    ) -> StandInLlm {
        StandInLlm {
            model: "stand-in".to_owned(),
            rule: Arc::new(rule),
            record: Arc::new(Mutex::new(Record::default())),
        }
    }

    pub fn named(self, model: &str) -> StandInLlm {
        StandInLlm {
            model: model.to_owned(),
            ..self
        }
    }

    pub fn questions(&self) -> Vec<AskedQuestion> {
        self.locked_record().questions.clone()
    }

    pub fn calls(&self) -> usize {
        self.locked_record().questions.len()
    }

    pub fn most_calls_at_once(&self) -> usize {
        self.locked_record().most_open
    }

    fn locked_record(&self) -> MutexGuard<'_, Record> {
        self.record
            .lock()
            .expect("the lock of the record should not be poisoned")
    }
}

impl Llm for StandInLlm {
    fn model(&self) -> &str {
        &self.model
    }

    async fn ask(&self, question: Question<'_>) -> Result<Value, LlmError> {
        let asked = AskedQuestion {
            system_prompt: question.system_prompt.to_owned(),
            schema: question.schema.to_owned(),
            input: question.input.to_owned(),
        };
        let earlier_calls = {
            let mut record = self.locked_record();
            record.questions.push(asked.clone());
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
        self.locked_record().open -= 1;
        (self.rule)(&asked, earlier_calls)
    }
}
