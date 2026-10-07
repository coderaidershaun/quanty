//! Lists the stored documents, changes their labels and removes them.

use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{DocId, Event, Failure, LabelEdit, RequestId};

pub async fn load_catalogue<S: Services>(_cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    reply.send(Event::Catalogue {
        request,
        result: Err(Failure::not_built("the catalogue")),
    });
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
