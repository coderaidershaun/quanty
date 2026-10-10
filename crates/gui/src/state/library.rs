//! The catalogue of stored documents, and the rules for refreshing it, changing a document's own
//! tags, deleting a document or a whole media, and saving or editing a media.

use std::collections::BTreeMap;

use super::IngestJob;
use super::shared::{Shared, push_cancel};
use crate::contract::{
    Catalogue, Command, DocId, DocumentTagsEdit, Effect, Failure, Loadable, MediaEdit, NewMedia,
    NoticeKind, RequestId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Busy {
    Saving(RequestId),
    Deleting(RequestId),
}

/// The save of a new media that the Ingest tab asked for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MediaSave {
    #[default]
    Idle,
    Saving {
        media: NewMedia,
        id: RequestId,
    },
    Failed {
        media: NewMedia,
        failure: Failure,
    },
}

impl MediaSave {
    pub fn is_saving(&self) -> bool {
        matches!(self, MediaSave::Saving { .. })
    }
}

/// A change to the labels of a stored media.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MediaEditing {
    #[default]
    Idle,
    Saving {
        edit: MediaEdit,
        id: RequestId,
    },
    Failed {
        edit: MediaEdit,
        failure: Failure,
    },
}

impl MediaEditing {
    pub fn is_saving(&self) -> bool {
        matches!(self, MediaEditing::Saving { .. })
    }
}

/// The delete of a whole media.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MediaDelete {
    #[default]
    Idle,
    Deleting {
        title: String,
        id: RequestId,
        /// The documents of the media when the delete was sent. A catalogue that is read while the
        /// delete is on its way no longer lists the ones that are gone.
        docs: Vec<DocId>,
    },
    Failed {
        title: String,
        failure: Failure,
    },
}

impl MediaDelete {
    pub fn is_deleting(&self) -> bool {
        matches!(self, MediaDelete::Deleting { .. })
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
    pub media_save: MediaSave,
    pub media_edit: MediaEditing,
    pub media_delete: MediaDelete,
}

impl Library {
    /// True while a delete of a media or of any document is on its way.
    pub fn is_deleting(&self) -> bool {
        self.media_delete.is_deleting()
            || self
                .busy
                .values()
                .any(|busy| matches!(busy, Busy::Deleting(_)))
    }
}

// SMELL: this file holds the states of the library and every rule that changes them, and it is
// too long for one file. The rules of a delete are the part to move out first.
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

    /// The form of the tags asks this before it sends a save, because a save that is dropped here
    /// would close the form as if the tags were saved.
    pub fn can_set_document_tags(&self) -> bool {
        !self.ingest.is_running() && !self.library.is_deleting()
    }

    pub(super) fn set_document_tags(&mut self, edit: DocumentTagsEdit, effects: &mut Vec<Effect>) {
        if !self.can_set_document_tags() {
            return;
        }
        let request = self.issue_request();
        self.library.failures.remove(&edit.doc);
        self.library.busy.insert(edit.doc, Busy::Saving(request));
        effects.push(Effect::Send(Command::SetDocumentTags { request, edit }));
    }

    /// A delete removes what an ingest or another change may be writing, so none is taken while
    /// an ingest runs or while any other change of the library is on its way.
    pub fn can_delete(&self) -> bool {
        self.can_edit_media() && self.library.busy.is_empty()
    }

    pub(super) fn delete_document(&mut self, doc: DocId, effects: &mut Vec<Effect>) {
        if !self.can_delete() {
            return;
        }
        let request = self.issue_request();
        self.library.failures.remove(&doc);
        self.library.busy.insert(doc, Busy::Deleting(request));
        effects.push(Effect::Send(Command::DeleteDocument { request, doc }));
    }

    pub(super) fn delete_media(&mut self, title: String, effects: &mut Vec<Effect>) {
        if !self.can_delete() {
            return;
        }
        let request = self.issue_request();
        let catalogue = self.library.catalogue.ready();
        let docs = catalogue
            .into_iter()
            .flat_map(|catalogue| catalogue.documents_of_media(&title))
            .map(|document| document.id)
            .collect();
        self.library.media_delete = MediaDelete::Deleting {
            title: title.clone(),
            id: request,
            docs,
        };
        effects.push(Effect::Send(Command::DeleteMedia { request, title }));
    }

    /// A save is ignored while a save or an edit of a media, or a delete, is on its way, so that no
    /// two of them race.
    pub(super) fn save_media(&mut self, media: NewMedia, effects: &mut Vec<Effect>) {
        if self.library.media_save.is_saving()
            || self.library.media_edit.is_saving()
            || self.library.is_deleting()
        {
            return;
        }
        let request = self.issue_request();
        self.library.media_save = MediaSave::Saving {
            media: media.clone(),
            id: request,
        };
        effects.push(Effect::Send(Command::SaveMedia { request, media }));
    }

    /// An edit of a media rewrites every document of it, so none is taken while an ingest runs or
    /// while a save, an edit or a delete is on its way.
    pub fn can_edit_media(&self) -> bool {
        !self.ingest.is_running()
            && !self.library.media_save.is_saving()
            && !self.library.media_edit.is_saving()
            && !self.library.is_deleting()
    }

    pub(super) fn edit_media(&mut self, edit: MediaEdit, effects: &mut Vec<Effect>) {
        if !self.can_edit_media() {
            return;
        }
        let request = self.issue_request();
        self.library.media_edit = MediaEditing::Saving {
            edit: edit.clone(),
            id: request,
        };
        effects.push(Effect::Send(Command::EditMedia { request, edit }));
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

    pub(super) fn document_tags_saved(
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

    pub(super) fn media_saved(
        &mut self,
        request: RequestId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        let MediaSave::Saving { media, id } = self.library.media_save.clone() else {
            return;
        };
        if id != request {
            return;
        }
        match result {
            Ok(()) => {
                self.library.media_save = MediaSave::Idle;
                self.cues.saved_media = Some(media.title);
                self.cues.media_saves += 1;
                self.refresh_catalogue(effects);
            }
            Err(failure) => {
                self.mark_down(&failure);
                self.library.media_save = MediaSave::Failed { media, failure };
            }
        }
    }

    pub(super) fn media_edited(
        &mut self,
        request: RequestId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        let MediaEditing::Saving { edit, id } = self.library.media_edit.clone() else {
            return;
        };
        if id != request {
            return;
        }
        match result {
            Ok(()) => {
                self.library.media_edit = MediaEditing::Idle;
                self.refresh_catalogue(effects);
            }
            Err(failure) => {
                self.mark_down(&failure);
                self.library.media_edit = MediaEditing::Failed { edit, failure };
            }
        }
    }

    pub(super) fn document_deleted(
        &mut self,
        request: RequestId,
        doc: DocId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        if self.library.busy.get(&doc) != Some(&Busy::Deleting(request)) {
            return;
        }
        self.drop_the_check(effects);
        self.library.busy.remove(&doc);
        let document = self
            .library
            .catalogue
            .ready()
            .and_then(|catalogue| catalogue.document(doc));
        let title = document.map(|document| document.title.clone());
        let removed = document.map(|document| document.items.to_string());
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
                // The document may be gone already, or gone in part.
                self.refresh_catalogue(effects);
            }
        }
    }

    pub(super) fn media_deleted(
        &mut self,
        request: RequestId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        let MediaDelete::Deleting { title, id, docs } = self.library.media_delete.clone() else {
            return;
        };
        if id != request {
            return;
        }
        self.drop_the_check(effects);
        match result {
            Ok(()) => {
                self.library.media_delete = MediaDelete::Idle;
                for doc in &docs {
                    self.library.failures.remove(doc);
                    self.close_source_of(*doc, effects);
                }
                let detail = match docs.len() {
                    0 => "It had no document.".to_owned(),
                    1 => "Removed 1 document.".to_owned(),
                    count => format!("Removed {count} documents."),
                };
                self.add_notice(NoticeKind::Done, format!("Deleted {title}"), detail, None);
                self.refresh_catalogue(effects);
            }
            Err(failure) => {
                self.mark_down(&failure);
                self.add_notice(
                    NoticeKind::Failed,
                    format!("Could not delete {title}"),
                    failure.hint.clone(),
                    Some(failure.clone()),
                );
                self.library.media_delete = MediaDelete::Failed { title, failure };
                // The documents before the one that failed are already gone.
                self.refresh_catalogue(effects);
            }
        }
    }

    /// A check says what is converted and what is ingested, and a delete changes both. So the
    /// check is dropped, and the Ingest page checks its draft again when it is drawn.
    fn drop_the_check(&mut self, effects: &mut Vec<Effect>) {
        let is_a_check = matches!(
            self.ingest,
            IngestJob::Checking { .. } | IngestJob::Checked { .. } | IngestJob::CheckFailed { .. }
        );
        if is_a_check {
            self.clear_ingest(effects);
        }
    }

    pub(super) fn library_is_busy(&self) -> bool {
        self.library.pending.is_some()
            || self.library.catalogue.is_loading()
            || !self.library.busy.is_empty()
            || self.library.media_save.is_saving()
            || self.library.media_edit.is_saving()
            || self.library.media_delete.is_deleting()
    }
}
