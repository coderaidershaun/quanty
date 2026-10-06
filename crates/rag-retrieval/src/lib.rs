//! Finds stored items for a question, and scores how well it does.

mod eval;
mod search;

pub use eval::{
    EvalReport, GoldenError, GoldenQuestion, QuestionOutcome, evaluate, read_golden_questions,
};
pub use search::{Retriever, SearchError, SearchResults};
