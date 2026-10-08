//! The catalogue of stored documents, and the rules for refreshing it, relabelling a document,
//! deleting one and saving a new book.

use std::collections::BTreeMap;

use super::shared::{Shared, counts_text, push_cancel};
use crate::contract::{
    Catalogue, Command, DocId, Effect, Failure, LabelEdit, Loadable, NewBook, NoticeKind, RequestId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Busy {
    Saving(RequestId),
    Deleting(RequestId),
}

/// The save of a new book that the Ingest tab asked for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum BookSave {
    #[default]
    Idle,
    Saving {
        book: NewBook,
        id: RequestId,
    },
    Failed {
        book: NewBook,
        failure: Failure,
    },
}

impl BookSave {
    pub fn is_saving(&self) -> bool {
        matches!(self, BookSave::Saving { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Library {
    pub catalogue: Loadable<Catalogue>,
    /// Goes up by one when `catalogue` changes. Whatever a panel works out from the catalogue is
    /// worked out again when this number changes.
    pub revision: u64,
    /// `Some` while a load runs. The old catalogue stays shown.
    pub pending: Option<RequestId>,
    pub busy: BTreeMap<DocId, Busy>,
    /// The last failed save or delete of each document.
    pub failures: BTreeMap<DocId, Failure>,
    pub book_save: BookSave,
}

impl Shared {
    pub(super) fn refresh_catalogue(&mut self, effects: &mut Vec<Effect>) {
        let request = self.issue_request();
        if let Some(old) = self.library.pending {
            push_cancel(effects, old);
        }
        self.library.pending = Some(request);
        if matches!(self.library.catalogue, Loadable::Idle | Loadable::Failed(_)) {
            self.library.catalogue = Loadable::Loading;
        }
        effects.push(Effect::Send(Command::LoadCatalogue { request }));
    }

    pub(super) fn set_labels(&mut self, edit: LabelEdit, effects: &mut Vec<Effect>) {
        if self.ingest.is_running() {
            return;
        }
        let request = self.issue_request();
        self.library.failures.remove(&edit.doc);
        self.library.busy.insert(edit.doc, Busy::Saving(request));
        effects.push(Effect::Send(Command::SetLabels { request, edit }));
    }

    pub(super) fn delete_document(&mut self, doc: DocId, effects: &mut Vec<Effect>) {
        if self.ingest.is_running() {
            return;
        }
        let request = self.issue_request();
        self.library.busy.insert(doc, Busy::Deleting(request));
        effects.push(Effect::Send(Command::DeleteDocument { request, doc }));
    }

    pub(super) fn save_book(&mut self, book: NewBook, effects: &mut Vec<Effect>) {
        if self.library.book_save.is_saving() {
            return;
        }
        let request = self.issue_request();
        self.library.book_save = BookSave::Saving {
            book: book.clone(),
            id: request,
        };
        effects.push(Effect::Send(Command::SaveBook { request, book }));
    }

    pub(super) fn catalogue_arrived(
        &mut self,
        request: RequestId,
        result: Result<Catalogue, Failure>,
    ) {
        if self.library.pending != Some(request) {
            return;
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        self.library.catalogue = result.into();
        self.library.revision += 1;
        self.library.pending = None;
    }

    pub(super) fn labels_saved(
        &mut self,
        request: RequestId,
        doc: DocId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        if self.library.busy.get(&doc) != Some(&Busy::Saving(request)) {
            return;
        }
        self.library.busy.remove(&doc);
        match result {
            Ok(()) => self.refresh_catalogue(effects),
            Err(failure) => {
                self.mark_down(&failure);
                self.library.failures.insert(doc, failure);
            }
        }
    }

    pub(super) fn book_saved(
        &mut self,
        request: RequestId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        let BookSave::Saving { book, id } = self.library.book_save.clone() else {
            return;
        };
        if id != request {
            return;
        }
        match result {
            Ok(()) => {
                self.library.book_save = BookSave::Idle;
                self.cues.saved_book = Some(book.title);
                self.cues.book_saves += 1;
                self.refresh_catalogue(effects);
            }
            Err(failure) => {
                self.mark_down(&failure);
                self.library.book_save = BookSave::Failed { book, failure };
            }
        }
    }

    pub(super) fn deleted(
        &mut self,
        request: RequestId,
        doc: DocId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        if self.library.busy.get(&doc) != Some(&Busy::Deleting(request)) {
            return;
        }
        self.library.busy.remove(&doc);
        let document = self
            .library
            .catalogue
            .ready()
            .and_then(|catalogue| catalogue.document(doc));
        let title = document.map(|document| document.title.clone());
        let removed = document.map(|document| counts_text(&document.items));
        match result {
            Ok(()) => {
                self.library.failures.remove(&doc);
                let title = title.map_or("Deleted a document".to_owned(), |title| {
                    format!("Deleted {title}")
                });
                let detail = removed.map_or(String::new(), |counts| format!("Removed {counts}."));
                self.add_notice(NoticeKind::Done, title, detail, None);
                self.close_source_of(doc, effects);
                self.refresh_catalogue(effects);
            }
            Err(failure) => {
                self.mark_down(&failure);
                let title = title.map_or("Could not delete a document".to_owned(), |title| {
                    format!("Could not delete {title}")
                });
                self.add_notice(
                    NoticeKind::Failed,
                    title,
                    failure.hint.clone(),
                    Some(failure.clone()),
                );
                self.library.failures.insert(doc, failure);
            }
        }
    }

    pub(super) fn library_is_busy(&self) -> bool {
        self.library.pending.is_some()
            || self.library.catalogue.is_loading()
            || !self.library.busy.is_empty()
            || self.library.book_save.is_saving()
    }
}
