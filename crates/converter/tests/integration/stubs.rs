//! Stand-ins for the two paid services, so the whole chapter run can be tried offline. Poppler,
//! the file system and every module of the crate stay real. There are no tests in this file.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use converter::ChapterJob;
use converter::categorise::PageCategories;
use converter::claude::{Answer, CallUsage};
use converter::content::Symbol;
use converter::jev::{JevError, MathPlacement};
use converter::services::{PageServices, PageSource, ServiceError};
use converter::transcribe::{
    CitedKind, CitedLabel, CopiedPage, CopiedPiece, Discussion, TranscribedPage, TranscribedPiece,
};

pub fn sample_pdf() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/chapter-1-sample-pages.pdf")
}

pub fn sample_job(output_root: &Path) -> ChapterJob {
    ChapterJob::new("Option Volatility and Pricing", &sample_pdf(), output_root).unwrap()
}

pub fn read_json(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// One call the run made through the services.
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

/// How the stub behaves page by page.
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
        output_tokens: 10,
        thinking_tokens: 0,
        seconds: 0.1,
    }
}

/// The page's position, except that page 6 shows 9 and page 7 shows 10, so exactly one page is
/// out of sequence.
fn printed_number(position: u32) -> Option<String> {
    Some(match position {
        6 => "9".to_owned(),
        7 => "10".to_owned(),
        other => other.to_string(),
    })
}

fn text(number: u32, markdown: &str, cites: Vec<CitedLabel>) -> TranscribedPiece {
    TranscribedPiece::Text {
        number,
        markdown: markdown.to_owned(),
        cites,
    }
}

fn formula(number: u32, latex: &str, label: &str) -> TranscribedPiece {
    TranscribedPiece::Formula {
        number,
        latex: latex.to_owned(),
        label: Some(label.to_owned()),
        name: None,
        statement: "The first quantity equals the second plus the third.".to_owned(),
        symbols: vec![Symbol {
            symbol: "a".to_owned(),
            meaning: "the first quantity".to_owned(),
        }],
    }
}

fn page_of(position: u32, pieces: Vec<TranscribedPiece>) -> TranscribedPage {
    TranscribedPage {
        printed_page_number: printed_number(position),
        running_header: None,
        pieces,
        discusses: Vec::new(),
        starts_mid_sentence: false,
        ends_mid_sentence: false,
    }
}

// Keep this page small. Its copied words are compared with the text layer of every sample page,
// and the tests expect that match to come out low on all of them. That only holds while the
// words copied here stay under about 150.
fn standard_transcription(position: u32) -> TranscribedPage {
    let pieces = vec![
        TranscribedPiece::Heading {
            number: 1,
            rank: 2,
            printed_number: None,
            text: "A Canned Heading".to_owned(),
        },
        text(
            2,
            "The price is given by",
            vec![CitedLabel {
                kind: CitedKind::Equation,
                label: "(2.1)".to_owned(),
            }],
        ),
        formula(3, "a = b + c", "(2.1)"),
        text(
            4,
            "The chart shows the result and the term is noted.[^1]",
            vec![CitedLabel {
                kind: CitedKind::Figure,
                label: "Figure 3-1".to_owned(),
            }],
        ),
        TranscribedPiece::Figure {
            number: 5,
            label: Some("Figure 3-1".to_owned()),
            caption: Some("Growth.".to_owned()),
            printed_text: vec!["Growth".to_owned(), "Time".to_owned()],
            explanation: "Figure 3-1, Growth. A line chart with time along the bottom axis and \
                growth up the side. The line starts low on the left, climbs steadily through the \
                middle of the chart and flattens near the top right. The title reads Growth and \
                the bottom axis is labelled Time. The chart shows growth that slows over time, so early gains are large and later gains are small, and the line never turns down anywhere on the page."
                .to_owned(),
        },
        TranscribedPiece::Table {
            number: 6,
            label: Some("Table 3-1".to_owned()),
            caption: None,
            markdown: "| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |".to_owned(),
            note: None,
            summary: "One row of three numbers.".to_owned(),
        },
        TranscribedPiece::Footnote {
            number: 7,
            marker: Some("1".to_owned()),
            markdown: "A note about the term.".to_owned(),
            cites: Vec::new(),
        },
    ];
    TranscribedPage {
        discusses: vec![Discussion { piece: 4, about: 5 }],
        ..page_of(position, pieces)
    }
}

fn page_seven_transcription() -> TranscribedPage {
    page_of(
        7,
        vec![
            formula(1, "x = y", "(7.9)"),
            text(2, "where the symbols are as before", Vec::new()),
        ],
    )
}

fn broken_transcription(position: u32) -> TranscribedPage {
    page_of(position, vec![formula(1, "\\frac{a", "(1.1)")])
}

fn copy_page_of(position: u32, words: &str) -> CopiedPage {
    CopiedPage {
        needs_stronger_model: false,
        printed_page_number: printed_number(position),
        running_header: None,
        pieces: vec![CopiedPiece::Text {
            number: 1,
            markdown: words.to_owned(),
            cites: Vec::new(),
        }],
        starts_mid_sentence: false,
        ends_mid_sentence: false,
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
            Scenario::SampleChapter if page.position == 7 => page_seven_transcription(),
            Scenario::SampleChapter if page.position == 6 => TranscribedPage {
                ends_mid_sentence: true,
                ..standard_transcription(6)
            },
            _ => standard_transcription(page.position),
        };
        Ok(Answer {
            value,
            usage: usage("stub-transcriber"),
        })
    }
}
