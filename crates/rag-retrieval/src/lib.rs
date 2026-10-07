//! Finds the stored items for a question: the nearest ones, and those that the graph leads to. It
//! can say what each step of the search produced, and it writes an answer from the items it found.

mod answer;
mod search;

pub use answer::{ANSWER_MODEL, Answer, AnswerError, Claim, Source, answer};
pub use search::{
    MAX_RESULTS_PER_DOCUMENT, RESULTS_PER_QUERY, Reason, Retriever, SearchError, SearchHit,
    SearchResults, SearchTrace, TracedSearch,
};
