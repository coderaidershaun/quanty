//! Reads a page and its concepts from a saved chapter.

use std::path::PathBuf;

use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{DocId, Event, Failure, RequestId};

/// The page to read, and the folder of its chapter if the catalogue knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTarget {
    pub doc: DocId,
    pub page: u32,
    pub folder: Option<PathBuf>,
}

pub async fn load_page<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    _target: &PageTarget,
    reply: &Reply,
) {
    let failure = Failure::not_built("reading a page");
    reply.send(Event::Page {
        request,
        result: Err(failure.clone()),
    });
    reply.send(Event::PageConcepts {
        request,
        result: Err(failure),
    });
}
