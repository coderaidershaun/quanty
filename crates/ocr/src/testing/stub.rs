//! The stand-ins for the paid calls of a page: each answers from a canned page and writes the
//! call down, so a test can say which calls a run made.

use std::sync::Mutex;

use crate::content::PageCategories;
use crate::convert::reply::{CopiedPage, TranscribedPage};
use crate::convert::services::{
    Answer, CallUsage, JevError, MathPlacement, PageServices, PageSource, ServiceError,
};

use super::pages::{
    broken_transcription, copy_page_of, sample_chapter_transcription, standard_transcription,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Tag(u32),
    Math(u32),
    Copy(u32),
    Transcribe {
        position: u32,
        correction: Option<String>,
    },
}

impl Call {
    pub fn position(&self) -> u32 {
        match self {
            Call::Tag(position) | Call::Math(position) | Call::Copy(position) => *position,
            Call::Transcribe { position, .. } => *position,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Scenario {
    /// Each page of the sample chapter reaches a different branch of the run.
    SampleChapter,
    /// Every page is tagged as holding a table, so every page goes to Sonnet. Sonnet's answers
    /// for `broken_page` always fail the reply check.
    AllTables { broken_page: Option<u32> },
}

pub struct StubServices {
    scenario: Scenario,
    calls: Mutex<Vec<Call>>,
}

fn usage(model: &str) -> CallUsage {
    CallUsage {
        model: model.to_owned(),
        cost_usd: 0.01,
        input_tokens: 100,
        output_tokens: 10,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        thinking_tokens: 0,
        seconds: 0.1,
    }
}

impl StubServices {
    pub fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    pub fn calls_for(&self, position: u32) -> Vec<Call> {
        let calls = self.calls();
        calls
            .into_iter()
            .filter(|call| call.position() == position)
            .collect()
    }

    fn log(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }
}

impl PageServices for StubServices {
    async fn tag(&self, page: &PageSource) -> Result<Answer<PageCategories>, ServiceError> {
        self.log(Call::Tag(page.position));
        let reports_table = match self.scenario {
            Scenario::SampleChapter => page.position == 6,
            Scenario::AllTables { .. } => true,
        };
        Ok(Answer {
            value: PageCategories {
                table: reports_table,
                ..PageCategories::default()
            },
            usage: usage("stub-tagger"),
        })
    }

    async fn contains_math(
        &self,
        page: &PageSource,
    ) -> Result<Option<MathPlacement>, ServiceError> {
        self.log(Call::Math(page.position));
        match self.scenario {
            Scenario::SampleChapter if page.position == 7 => {
                Err(ServiceError::Jev(JevError::Rejected {
                    status: 503,
                    body: "stub outage".to_owned(),
                }))
            }
            _ => Ok(None),
        }
    }

    async fn copy(&self, page: &PageSource) -> Result<Answer<CopiedPage>, ServiceError> {
        self.log(Call::Copy(page.position));
        // The text layer's own words, with the soft hyphens it carries still in them, as a real
        // model's copy could have.
        let words: Vec<&str> = page.text_layer.split_whitespace().collect();
        let copy = match page.position {
            2 => CopiedPage {
                needs_stronger_model: true,
                pieces: Vec::new(),
                printed_page_number: None,
                ..copy_page_of(2, "")
            },
            3 => copy_page_of(3, &format!("{} \\alpha", words.join(" "))),
            4 => copy_page_of(4, &words[..words.len() / 2].join(" ")),
            5 => {
                let invented: Vec<String> = (0..100).map(|n| format!("invented{n}")).collect();
                copy_page_of(5, &format!("{} {}", words.join(" "), invented.join(" ")))
            }
            position => copy_page_of(position, &words.join(" ")),
        };
        Ok(Answer {
            value: copy,
            usage: usage("stub-copier"),
        })
    }

    async fn transcribe(
        &self,
        page: &PageSource,
        correction: Option<&str>,
    ) -> Result<Answer<TranscribedPage>, ServiceError> {
        self.log(Call::Transcribe {
            position: page.position,
            correction: correction.map(str::to_owned),
        });
        let value = match self.scenario {
            Scenario::AllTables {
                broken_page: Some(broken),
            } if broken == page.position => broken_transcription(page.position),
            Scenario::SampleChapter => {
                sample_chapter_transcription(page.position, correction.is_some())
            }
            _ => standard_transcription(page.position),
        };
        Ok(Answer {
            value,
            usage: usage("stub-transcriber"),
        })
    }
}
