//! Finds the stored items for a question: the nearest ones, and those that the graph leads to. It
//! also writes an answer from the items it found, and scores how well the search does.

mod answer;
mod eval;
mod search;

pub use answer::{ANSWER_MODEL, Answer, AnswerError, Claim, answer};
pub use eval::{
    EvalReport, GoldenError, GoldenPlace, GoldenQuestion, QuestionOutcome, evaluate,
    read_golden_questions,
};
pub use search::{
    MAX_RESULTS_PER_DOCUMENT, RESULTS_PER_QUERY, Reason, Retriever, SearchError, SearchHit,
    SearchResults,
};
