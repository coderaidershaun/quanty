//! The read of the two accounts, answered with what the scene says it finds.

use std::time::Duration;

use super::{Fake, Reply};
use contract::{Event, RequestId};

/// Long enough for the footer to be seen waiting before it shows what it found.
const BALANCE_WAIT: Duration = Duration::from_millis(300);

impl Fake {
    pub(super) async fn read_balances(&self, request: RequestId, reply: &Reply) {
        self.wait(BALANCE_WAIT).await;
        reply.send(Event::Balances {
            request,
            found: self.scene.script.balances,
        });
    }
}
