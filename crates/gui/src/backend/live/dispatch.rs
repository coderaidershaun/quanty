//! Hands each command to the part of the live backend that does it. Each part sends its own
//! events.

use super::context::{LiveContext, Services};
use super::source::PageTarget;
use super::{ingest, library, query, source};
use crate::backend::{Handler, Reply};
use crate::contract::Command;

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
            Command::SetLabels { request, edit } => {
                library::set_labels(self, request, &edit, &reply).await;
            }
            Command::DeleteDocument { request, doc } => {
                library::delete(self, request, doc, &reply).await;
            }
            Command::Preflight { request, ingest } => {
                ingest::preflight(self, request, &ingest, &reply).await;
            }
            Command::Ingest { request, ingest } => {
                ingest::run(self, request, &ingest, &reply).await;
            }
            Command::CheckHealth { request } => {
                ingest::check_health(self, request, &reply).await;
            }
            Command::Cancel(_) => {}
        }
    }
}
