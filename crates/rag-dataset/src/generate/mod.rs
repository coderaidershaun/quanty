//! Writes the synthetic texts of a dataset: the rewrites, explanations of relations and exercises
//! that models write from the sections the plan chose. Nothing is written yet.

pub mod asker;
pub mod role;

use rag_core::{Llm, UsageTally};

use crate::layout::{DatasetFolder, WriteError};
use crate::options::BuildOptions;
use crate::plan::GenerationPlan;
use crate::progress::{Step, Written};
use crate::section::Section;

use asker::{AskError, Asker};

#[derive(thiserror::Error, Debug)]
pub enum GenerateError {
    #[error(transparent)]
    Ask(#[from] AskError),

    #[error(transparent)]
    Write(#[from] WriteError),
}

/// Not built yet: writes nothing and says so.
///
/// # Errors
/// [`GenerateError::Ask`] when the model can answer no question, and [`GenerateError::Write`] when
/// a file cannot be written.
pub async fn write_synthetic<L: Llm>(
    _plan: &GenerationPlan,
    _sections: &[Section],
    _asker: &Asker<'_, L>,
    _folder: &DatasetFolder,
    _options: &BuildOptions,
    _on_step: &mut (dyn FnMut(Step, &UsageTally) + Send),
) -> Result<Written, GenerateError> {
    tracing::info!("the synthetic texts are not built yet, so none are written");
    Ok(Written::default())
}
