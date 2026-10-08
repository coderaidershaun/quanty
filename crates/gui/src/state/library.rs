//! The catalogue of stored documents, and the rules for refreshing it, changing a document's own
//! tags, deleting a document, and saving or editing a media.

use std::collections::BTreeMap;

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

    pub(super) fn set_document_tags(&mut self, edit: DocumentTagsEdit, effects: &mut Vec<Effect>) {
        if self.ingest.is_running() {
            return;
        }
        let request = self.issue_request();
        self.library.failures.remove(&edit.doc);
        self.library.busy.insert(edit.doc, Busy::Saving(request));
        effects.push(Effect::Send(Command::SetDocumentTags { request, edit }));
    }

    pub(super) fn delete_document(&mut self, doc: DocId, effects: &mut Vec<Effect>) {
        if self.ingest.is_running() {
            return;
        }
        let request = self.issue_request();
        self.library.busy.insert(doc, Busy::Deleting(request));
        effects.push(Effect::Send(Command::DeleteDocument { request, doc }));
    }

    /// A save is ignored while a save or an edit of a media is in flight, so the two never race.
    pub(super) fn save_media(&mut self, media: NewMedia, effects: &mut Vec<Effect>) {
        if self.library.media_save.is_saving() || self.library.media_edit.is_saving() {
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
    /// while a save or an edit of a media is on its way.
    pub fn can_edit_media(&self) -> bool {
        !self.ingest.is_running()
            && !self.library.media_save.is_saving()
            && !self.library.media_edit.is_saving()
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
            }
        }
    }

    pub(super) fn library_is_busy(&self) -> bool {
        self.library.pending.is_some()
            || self.library.catalogue.is_loading()
            || !self.library.busy.is_empty()
            || self.library.media_save.is_saving()
            || self.library.media_edit.is_saving()
    }
}
