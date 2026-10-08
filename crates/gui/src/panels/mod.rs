//! The panels of the window. Each draws from the shared state and its own small state, and
//! answers with intents: none of them changes the shared state or touches a store.

pub mod answer;
pub mod ask_bar;
pub mod concept_graph;
pub mod follow_up;
pub mod ingest;
mod labels;
pub mod library;
mod media_card;
pub mod notices;
pub mod retrieval_path;
mod seam;
pub mod source;

pub use seam::{Locals, PanelCx, placeholder};
