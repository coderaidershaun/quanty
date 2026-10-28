//! Tells connected agents that the list of resources changed: on a `subscriptions/listen` stream
//! for a peer of the 2026-07-28 protocol, and as a notice on the session of an older peer.

use rmcp::ErrorData;
use rmcp::RoleServer;
use rmcp::model::SubscriptionFilter;
use rmcp::service::{Peer, SubscriptionContext};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::library::LibraryChanges;

/// What a listener can ask for: only that the list of resources changed.
pub(crate) fn accepted_filter() -> SubscriptionFilter {
    SubscriptionFilter::builder()
        .resources_list_changed()
        .build()
}

/// Sends a notice on the stream of `subscription` for each change of the library, until the
/// client cancels the stream or it cannot be reached.
pub(crate) async fn listen(
    changes: &LibraryChanges,
    subscription: SubscriptionContext,
) -> Result<(), ErrorData> {
    let mut changed = changes.subscribe();
    loop {
        tokio::select! {
            () = subscription.cancelled() => return Ok(()),
            again = changed_again(&mut changed) => {
                if !again || subscription.sink().notify_resource_list_changed().await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

/// Sends a notice to `peer` for each change of the library from now on. The task ends when the
/// notice cannot be sent, so after the peer has gone it lives until the next change.
pub(crate) fn tell_session(changes: &LibraryChanges, peer: Peer<RoleServer>) {
    let mut changed = changes.subscribe();
    tokio::spawn(async move {
        while changed_again(&mut changed).await {
            if peer.notify_resource_list_changed().await.is_err() {
                return;
            }
        }
    });
}

/// Waits for the next change of the library. It is `false` when the library can no longer
/// announce one.
async fn changed_again(changed: &mut broadcast::Receiver<()>) -> bool {
    match changed.recv().await {
        // A listener that missed some changes lists the library again all the same.
        Ok(()) | Err(RecvError::Lagged(_)) => true,
        Err(RecvError::Closed) => false,
    }
}
