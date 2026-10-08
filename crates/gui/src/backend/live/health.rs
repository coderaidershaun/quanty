//! Answers the window when it asks what is known about each service the app needs.

use super::{LiveContext, Services};
use crate::backend::Reply;
use crate::contract::{Event, RequestId, Service, ServiceState};

/// Not built yet. It says "not known" and not "down": while the app believes a store is down it
/// checks again after every search that works, and a check that always said "down" would keep it
/// checking for ever.
pub async fn check_health<S: Services>(_cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    for service in Service::ALL {
        reply.send(Event::Health {
            request,
            service,
            state: ServiceState::Unknown,
        });
    }
}
