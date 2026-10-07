//! Lists the stored documents, changes their labels and removes them.

mod catalogue;
mod chapters;

use graph::GraphStore;

use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{Catalogue, DocId, Event, Failure, LabelEdit, RequestId};

pub(in crate::backend::live) use chapters::chapters_on_disk;

/// Sends exactly one catalogue, also when it cannot be read, so the window never waits for one.
pub async fn load_catalogue<S: Services>(cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    let result = read_catalogue(cx).await;
    reply.send(Event::Catalogue { request, result });
}

/// The stored documents, each with its chapter on disk. Of the stores only the graph is read, so
/// a vector store that is down does not stop the list.
async fn read_catalogue<S: Services>(cx: &LiveContext<S>) -> Result<Catalogue, Failure> {
    let graph = cx.graph().await?;
    let records = graph
        .document_records()
        .await
        .map_err(|error| cx.failure(error))?;
    let found = chapters_on_disk(
        records
            .iter()
            .map(|record| (record.node.id, record.chapter_folder.as_deref())),
        &cx.config().content_folder,
    );
    Ok(catalogue::catalogue_from(records, found))
}

pub async fn set_labels<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    edit: &LabelEdit,
    reply: &Reply,
) {
    reply.send(Event::LabelsSaved {
        request,
        doc: edit.doc,
        result: Err(Failure::not_built("changing labels")),
    });
}

pub async fn delete<S: Services>(
    _cx: &LiveContext<S>,
    request: RequestId,
    doc: DocId,
    reply: &Reply,
) {
    reply.send(Event::Deleted {
        request,
        doc,
        result: Err(Failure::not_built("deleting a document")),
    });
}
