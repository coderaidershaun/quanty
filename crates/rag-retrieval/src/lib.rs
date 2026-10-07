//! Finds the stored items for a question: the nearest ones, and those that the graph leads to. It
//! also writes an answer from the items it found.

mod answer;
mod search;

pub use answer::{ANSWER_MODEL, Answer, AnswerError, Claim, answer};
pub use search::{
    MAX_RESULTS_PER_DOCUMENT, RESULTS_PER_QUERY, Reason, Retriever, SearchError, SearchHit,
    SearchResults,
};
