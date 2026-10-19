//! Builds Gemma 4 training data from the converted documents: whole book sections for continued
//! pre-training, synthetic rewrites budgeted by concept rarity, verified conversations for
//! fine-tuning, and one manifest that says what the dataset holds.

pub mod checks;
pub mod documents;
pub mod generate;
pub mod layout;
pub mod manifest;
pub mod options;
pub mod plan;
pub mod progress;
pub mod sample;
pub mod section;
pub mod sft;

mod build;

pub use build::{BuildError, build};
pub use layout::DatasetFolder;
pub use options::{BuildOptions, Selection};
pub use progress::{GenerationKind, Report, SplitCounts, Step, Written};
