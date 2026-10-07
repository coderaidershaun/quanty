//! Answers a question from the stored items: searches, reads the graph and writes the answer.

use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{AskDraft, Event, Failure, RequestId};

pub async fn ask<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    _ask: &AskDraft,
    reply: &Reply,
) {
    reply.send(Event::Search {
        request,
        result: Err(Failure::not_built("search")),
    });
}
