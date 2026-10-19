//! Writes the fine-tuning examples of a dataset: questions that models write from the sections,
//! the answers they give, and the checks that decide which answers are kept. Nothing is written
//! yet.

use rag_core::{Llm, UsageTally};

use crate::generate::asker::{AskError, Asker};
use crate::layout::{DatasetFolder, WriteError};
use crate::options::BuildOptions;
use crate::plan::GenerationPlan;
use crate::progress::{Step, Written};
use crate::section::Section;

#[derive(thiserror::Error, Debug)]
pub enum SftError {
    #[error(transparent)]
    Ask(#[from] AskError),

    #[error(transparent)]
    Write(#[from] WriteError),
}

/// Not built yet: writes nothing and says so.
///
/// # Errors
/// [`SftError::Ask`] when the model can answer no question, and [`SftError::Write`] when a file
/// cannot be written.
pub async fn write_sft<L: Llm>(
    _plan: &GenerationPlan,
    _sections: &[Section],
    _asker: &Asker<'_, L>,
    _folder: &DatasetFolder,
    _options: &BuildOptions,
    _on_step: &mut (dyn FnMut(Step, &UsageTally) + Send),
) -> Result<Written, SftError> {
    tracing::info!("the fine-tuning examples are not built yet, so none are written");
    Ok(Written::default())
}
