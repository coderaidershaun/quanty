//! The data the scenes answer with: a catalogue, the pages and their concepts, an ask and its
//! reply, and the steps of an ingest.

mod ask;
mod concepts;
pub(super) mod ingest;
mod library;

pub(super) use ask::{
    QUESTION, answer, answer_without_blocks, graph, no_document_has_the_labels, no_result, reply,
    reply_without_concepts,
};
pub(super) use concepts::page_concepts;
pub(super) use library::{SampleError, catalogue, find_samples, page};
