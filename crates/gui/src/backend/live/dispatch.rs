//! Hands each command to the part of the live backend that does it. Each part sends its own
//! events.

use super::context::{LiveContext, Services};
use super::source::PageTarget;
use super::{ingest, library, query, source};
use crate::backend::{Handler, Reply};
use crate::contract::{Command, RequestId, unknown_health};

impl<S: Services> Handler for LiveContext<S> {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::Ask { request, ask } => query::ask(self, request, &ask, &reply).await,
            Command::LoadPage {
                request,
                doc,
                page,
                folder,
            } => {
                let target = PageTarget { doc, page, folder };
                source::load_page(self, request, &target, &reply).await;
            }
            Command::LoadCatalogue { request } => {
                library::load_catalogue(self, request, &reply).await;
            }
            Command::SetDocumentTags { request, edit } => {
                library::set_document_tags(self, request, &edit, &reply).await;
            }
            Command::DeleteDocument { request, doc } => {
                library::delete_document(self, request, doc, &reply).await;
            }
            Command::SaveMedia { request, media } => {
                library::save_media(self, request, &media, &reply).await;
            }
            Command::EditMedia { request, edit } => {
                library::edit_media(self, request, &edit, &reply).await;
            }
            Command::DeleteMedia { request, title } => {
                library::delete_media(self, request, &title, &reply).await;
            }
            Command::Preflight { request, ingest } => {
                ingest::preflight(self, request, &ingest, &reply).await;
            }
            Command::Ingest { request, ingest } => {
                ingest::run(self, request, &ingest, &reply).await;
            }
            Command::CheckHealth { request } => check_health(request, &reply),
            Command::Cancel(_) => {}
        }
    }
}

/// Not built yet. It says "not known" and not "down": while the app believes a store is down it
/// checks again after every search that works, and a check that always said "down" would keep it
/// checking for ever.
fn check_health(request: RequestId, reply: &Reply) {
    for event in unknown_health(request) {
        reply.send(event);
    }
}
