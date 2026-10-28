//! Tells every connected agent that the stored library changed, so that it lists the library
//! again.

use tokio::sync::broadcast;

/// A notice can wait for a slow listener. A listener that falls further behind learns only that
/// it missed some, which is all it needs to list the library again.
const WAITING_NOTICES: usize = 16;

/// The place where a change of the library is announced. A clone announces to and listens with
/// the same set of listeners, so a server that hosts the MCP server hands it a clone and
/// announces the changes that happen outside the tools.
#[derive(Clone, Debug)]
pub struct LibraryChanges(broadcast::Sender<()>);

impl Default for LibraryChanges {
    fn default() -> LibraryChanges {
        LibraryChanges::new()
    }
}

impl LibraryChanges {
    /// A place with no listeners yet.
    pub fn new() -> LibraryChanges {
        LibraryChanges(broadcast::channel(WAITING_NOTICES).0)
    }

    /// Tells every listener that the library changed.
    pub fn notify(&self) {
        // The only error is that nobody listens, and then there is nobody to tell.
        self.0.send(()).ok();
    }

    /// A listener for the changes announced from now on.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<()> {
        self.0.subscribe()
    }
}
