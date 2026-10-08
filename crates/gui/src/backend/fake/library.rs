//! The catalogue that the fake keeps in memory, and the answers that read it or change it.

use std::sync::MutexGuard;

use super::scenes::Library;
use super::{CATALOGUE_WAIT, Fake};
use crate::backend::Reply;
use crate::contract::{Book, Catalogue, Event, Failure, LabelEdit, NewBook, RequestId};

pub(super) fn starting_catalogue(library: Library, samples: Catalogue) -> Catalogue {
    match library {
        Library::Samples => samples,
        Library::Empty | Library::Fails(_) => Catalogue::default(),
    }
}

impl Fake {
    /// Never hold the guard across an `await`.
    pub(super) fn catalogue(&self) -> MutexGuard<'_, Catalogue> {
        self.catalogue
            .lock()
            .expect("the catalogue of the fake is not poisoned")
    }

    /// Waits as a store would, then answers, or fails when the library of the scene fails.
    async fn after_wait<T>(
        &self,
        answer: impl FnOnce() -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        self.wait(CATALOGUE_WAIT).await;
        match self.scene.script.library {
            Library::Fails(kind) => Err(self.failure(kind)),
            Library::Samples | Library::Empty => answer(),
        }
    }

    pub(super) async fn load_catalogue(&self, request: RequestId, reply: &Reply) {
        let result = self.after_wait(|| Ok(self.catalogue().clone())).await;
        reply.send(Event::Catalogue { request, result });
    }

    pub(super) async fn save_book(&self, request: RequestId, book: &NewBook, reply: &Reply) {
        let result = self.after_wait(|| self.add_book(book)).await;
        reply.send(Event::BookSaved { request, result });
    }

    /// Adds the book the way the live backend stores it: the title and the author trimmed, a blank
    /// author none, and the tags in lower case, each once, in order.
    // SMELL: how a saved book is stored is written here and again in the live backend, which
    // does it with the types of the stores. A change to one must be made in both.
    fn add_book(&self, book: &NewBook) -> Result<(), Failure> {
        let mut catalogue = self.catalogue();
        if let Some(stored) = catalogue.stored_title(&book.title) {
            return Err(Failure::book_exists(stored));
        }
        let tags = to_stored(&book.tags);
        catalogue.books.push(Book {
            title: Some(book.title.trim().to_owned()),
            author: book
                .author
                .as_deref()
                .map(str::trim)
                .filter(|author| !author.is_empty())
                .map(str::to_owned),
            tags,
            chapters: Vec::new(),
        });
        catalogue.sort_books();
        Ok(())
    }

    pub(super) async fn set_labels(&self, request: RequestId, edit: &LabelEdit, reply: &Reply) {
        let result = self.after_wait(|| self.relabel(edit)).await;
        reply.send(Event::LabelsSaved {
            request,
            doc: edit.doc,
            result,
        });
    }

    /// A new author replaces the old one, the tags to add are added, and then the tags to remove
    /// are taken away. The tags end in lower case, each once, in order.
    // SMELL: how a change of labels is applied is written here and again in the ingestion crate,
    // which the fake may not name. A change to one must be made in both.
    fn relabel(&self, edit: &LabelEdit) -> Result<(), Failure> {
        let mut catalogue = self.catalogue();
        let document = catalogue
            .books
            .iter_mut()
            .flat_map(|book| book.chapters.iter_mut())
            .find(|document| document.id == edit.doc)
            .ok_or_else(|| {
                Failure::internal(format!("the library has no document {}", edit.doc.0))
            })?;
        if let Some(author) = &edit.author {
            document.author = Some(author.clone());
        }
        let removed = to_stored(&edit.remove);
        let mut tags = document.tags.clone();
        tags.extend(to_stored(&edit.add));
        tags.retain(|tag| !removed.contains(tag));
        document.tags = to_stored(&tags);
        Ok(())
    }
}

/// The tags as the stores keep them: lower case, none blank, each once, in order.
fn to_stored(tags: &[String]) -> Vec<String> {
    let mut stored: Vec<String> = tags
        .iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();
    stored.sort();
    stored.dedup();
    stored
}
