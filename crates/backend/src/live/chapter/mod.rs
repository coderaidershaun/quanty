//! Everything the live backend reads from the chapter folder of a stored document: the folder
//! itself, its items, its saved insights and its pages.

mod contents;
mod folder;
mod insights;
pub mod source;

use contract::Failure;

pub(super) use contents::load_chapter_contents;
pub(super) use folder::chapters_on_disk;
pub(super) use insights::load_chapter_insights;

/// Runs `work` on a thread that may block. A fault of the work becomes the failure that `fault`
/// makes of it; a thread that died becomes an internal failure that names what stopped.
async fn blocking<T: Send + 'static, E: Send + 'static>(
    what: &str,
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
    fault: impl FnOnce(E) -> Failure,
) -> Result<T, Failure> {
    match tokio::task::spawn_blocking(work).await {
        Ok(done) => done.map_err(fault),
        Err(error) => Err(Failure::internal(format!("the {what} stopped: {error}"))),
    }
}
