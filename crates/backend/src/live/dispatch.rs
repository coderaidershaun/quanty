//! Hands each command to the part of the live backend that does it. Each part sends its own
//! events.

use super::context::{LiveContext, Services};
use super::source::PageTarget;
use super::{balances, chapter, datasets, health, ingest, library, query, source};
use crate::handler::save_model_choice;
use crate::{Handler, Reply, exporting};
use contract::{Command, Event};

impl<S: Services> Handler for LiveContext<S> {
    async fn serve(&self, command: Command, reply: Reply) {
        match command {
            Command::SaveModelChoice {
                request,
                work,
                choice,
            } => {
                let result = save_model_choice(self.home(), work, choice);
                reply.send(Event::ModelChoiceSaved { request, result });
            }
            Command::Ask { request, ask } => query::ask(self, request, &ask, &reply).await,
            Command::Export { request, file } => {
                exporting::write_file(
                    request,
                    file,
                    self.home(),
                    &self.config().content_folder,
                    &reply,
                )
                .await;
            }
            Command::LoadPage {
                request,
                doc,
                page,
                folder,
            } => {
                let target = PageTarget { doc, page, folder };
                source::load_page(self, request, &target, &reply).await;
            }
            Command::LoadChapterContents {
                request,
                doc,
                folder,
            } => {
                chapter::load_chapter_contents(self, request, doc, folder, &reply).await;
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
            Command::BuildDataset { request, build } => {
                datasets::build(self, request, &build, &reply).await;
            }
            Command::CheckHealth { request } => health::check(self, request, &reply).await,
            Command::LoadChapterInsights {
                request,
                doc,
                folder,
            } => chapter::load_chapter_insights(self, request, doc, folder, &reply).await,
            Command::Cancel(_) => {}
            Command::ReadBalances { request } => balances::read(self, request, &reply).await,
        }
    }
}
