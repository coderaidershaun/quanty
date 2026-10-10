//! The catalogue that the fake keeps in memory, and the answers that read it or change it.

use std::sync::MutexGuard;

use super::scenes::Library;
use super::{CATALOGUE_WAIT, Fake};
use crate::backend::Reply;
use crate::contract::{
    Catalogue, DocId, DocumentTagsEdit, Event, Failure, Media, MediaEdit, NewMedia, RequestId,
};

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

    pub(super) async fn save_media(&self, request: RequestId, media: &NewMedia, reply: &Reply) {
        let result = self.after_wait(|| self.add_media(media)).await;
        reply.send(Event::MediaSaved { request, result });
    }

    /// Adds the media the way the live backend stores it: the title and the authors trimmed, a
    /// blank author dropped, and the tags in lower case, each once, in order.
    // SMELL: the core crate keeps its own copy of this rule, and the fake may not name that crate,
    // so a change to one must be made in both.
    fn add_media(&self, media: &NewMedia) -> Result<(), Failure> {
        let mut catalogue = self.catalogue();
        if let Some(stored) = catalogue.stored_title(&media.title) {
            return Err(Failure::media_exists(stored));
        }
        catalogue.media.push(Media {
            title: Some(media.title.trim().to_owned()),
            category: media.category,
            authors: to_authors(&media.authors),
            tags: to_stored(&media.tags),
            documents: Vec::new(),
        });
        catalogue.sort_media();
        Ok(())
    }

    pub(super) async fn edit_media(&self, request: RequestId, edit: &MediaEdit, reply: &Reply) {
        let result = self.after_wait(|| self.relabel_media(edit)).await;
        reply.send(Event::MediaEdited { request, result });
    }

    /// The media takes the new category, authors and tags, and every document of it a copy of the
    /// authors and the tags.
    // SMELL: the ingestion crate keeps its own copy of this rule, and the fake may not name that
    // crate, so a change to one must be made in both.
    fn relabel_media(&self, edit: &MediaEdit) -> Result<(), Failure> {
        let mut catalogue = self.catalogue();
        let media = catalogue
            .media
            .iter_mut()
            .find(|media| media.is_titled(&edit.title))
            .ok_or_else(|| {
                Failure::internal(format!("the library has no media titled {}", edit.title))
            })?;
        media.category = edit.category;
        media.authors = to_authors(&edit.authors);
        media.tags = to_stored(&edit.tags);
        for document in &mut media.documents {
            document.authors = media.authors.clone();
            document.media_tags = media.tags.clone();
        }
        Ok(())
    }

    pub(super) async fn set_document_tags(
        &self,
        request: RequestId,
        edit: &DocumentTagsEdit,
        reply: &Reply,
    ) {
        let result = self.after_wait(|| self.retag(edit)).await;
        reply.send(Event::DocumentTagsSaved {
            request,
            doc: edit.doc,
            result,
        });
    }

    /// The tags to add are added, and then the tags to remove are taken away. The tags end in
    /// lower case, each once, in order.
    // SMELL: the ingestion crate keeps its own copy of this rule, and the fake may not name that
    // crate, so a change to one must be made in both.
    fn retag(&self, edit: &DocumentTagsEdit) -> Result<(), Failure> {
        let mut catalogue = self.catalogue();
        let document = catalogue
            .media
            .iter_mut()
            .flat_map(|media| media.documents.iter_mut())
            .find(|document| document.id == edit.doc)
            .ok_or_else(|| {
                Failure::internal(format!("the library has no document {}", edit.doc.0))
            })?;
        let removed = to_stored(&edit.remove);
        let mut tags = document.tags.clone();
        tags.extend(to_stored(&edit.add));
        tags.retain(|tag| !removed.contains(tag));
        document.tags = to_stored(&tags);
        Ok(())
    }

    pub(super) async fn delete_document(&self, request: RequestId, doc: DocId, reply: &Reply) {
        let result = self.after_wait(|| self.remove_document(doc)).await;
        reply.send(Event::DocumentDeleted {
            request,
            doc,
            result,
        });
    }

    /// A titled media stays when its last document goes, and the group with no title goes with its
    /// last document, both as in the live library. Only the catalogue in memory changes: the
    /// folders of the sample documents belong to the repository, and no delete may touch them.
    // SMELL: the ingestion crate keeps its own copy of this rule, and the fake may not name that
    // crate, so a change to one must be made in both.
    fn remove_document(&self, doc: DocId) -> Result<(), Failure> {
        let mut catalogue = self.catalogue();
        let media = catalogue
            .media
            .iter_mut()
            .find(|media| media.documents.iter().any(|document| document.id == doc))
            .ok_or_else(|| Failure::internal(format!("the library has no document {}", doc.0)))?;
        media.documents.retain(|document| document.id != doc);
        catalogue
            .media
            .retain(|media| media.title.is_some() || !media.documents.is_empty());
        Ok(())
    }

    pub(super) async fn delete_media(&self, request: RequestId, title: &str, reply: &Reply) {
        let result = self.after_wait(|| self.remove_media(title)).await;
        reply.send(Event::MediaDeleted { request, result });
    }

    /// Every media of that title goes with its documents, whatever the capitals. Only the
    /// catalogue in memory changes, as for a document.
    // SMELL: the ingestion crate keeps its own copy of this rule, and the fake may not name that
    // crate, so a change to one must be made in both.
    fn remove_media(&self, title: &str) -> Result<(), Failure> {
        let mut catalogue = self.catalogue();
        let before = catalogue.media.len();
        catalogue.media.retain(|media| !media.is_titled(title));
        if catalogue.media.len() == before {
            return Err(Failure::internal(format!(
                "the library has no media titled {title}"
            )));
        }
        Ok(())
    }
}

/// The authors as the stores keep them: trimmed, none blank, each once, in the order given.
fn to_authors(authors: &[String]) -> Vec<String> {
    let mut stored: Vec<String> = Vec::new();
    for author in authors.iter().map(|author| author.trim()) {
        if !author.is_empty() && !stored.iter().any(|kept| kept == author) {
            stored.push(author.to_owned());
        }
    }
    stored
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
