//! Checks a chapter before it is ingested, ingests it, and reports on the services it needs.

use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{Event, Failure, IngestRequest, RequestId, Service, ServiceState};

pub async fn preflight<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    _ingest: &IngestRequest,
    reply: &Reply,
) {
    reply.send(Event::Preflight {
        request,
        result: Err(Failure::not_built("checking a chapter")),
    });
}

pub async fn run<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    _ingest: &IngestRequest,
    reply: &Reply,
) {
    reply.send(Event::IngestFinished {
        request,
        result: Err(Failure::not_built("ingesting a chapter")),
    });
}

pub async fn check_health<S: Services>(_cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    let failure = Failure::not_built("the health check");
    for service in Service::ALL {
        reply.send(Event::Health {
            request,
            service,
            state: ServiceState::Down(failure.clone()),
        });
    }
}
