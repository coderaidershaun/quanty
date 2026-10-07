//! The catalogue of stored documents, and the rules for refreshing it, relabelling a document
//! and deleting one.

use std::collections::BTreeMap;

use super::shared::{Shared, counts_text, push_cancel};
use crate::contract::{
    Catalogue, Command, DocId, Effect, Failure, LabelEdit, Loadable, NoticeKind, RequestId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Busy {
    Saving(RequestId),
    Deleting(RequestId),
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
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::contract::{Book, Document, Event, FailureKind, IngestRequest, Intent, NoticeKind};
    use crate::state::IngestJob;

    fn doc(number: u128) -> DocId {
        DocId(Uuid::from_u128(number))
    }

    fn catalogue(doc: DocId) -> Catalogue {
        Catalogue {
            books: vec![Book {
                title: None,
                chapters: vec![Document {
                    id: doc,
                    title: "Notes chapter 1".to_owned(),
                    ..Document::default()
                }],
            }],
        }
    }

    fn run(shared: &mut Shared, intent: Intent) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_intent(intent, &mut effects);
        effects
    }

    fn deliver(shared: &mut Shared, event: Event) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_event(event, &mut effects);
        effects
    }

    fn requested(effects: &[Effect]) -> RequestId {
        effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Send(
                    Command::LoadCatalogue { request }
                    | Command::SetLabels { request, .. }
                    | Command::DeleteDocument { request, .. },
                ) => Some(*request),
                _ => None,
            })
            .expect("a library command was sent")
    }

    #[test]
    fn a_delete_reloads_the_catalogue_and_closes_its_open_source() {
        let target = doc(1);
        let mut shared = Shared::default();

        let effects = run(&mut shared, Intent::RefreshCatalogue);
        let first = requested(&effects);
        assert_eq!(effects.len(), 1);
        assert!(shared.library.catalogue.is_loading());
        assert_eq!(shared.library.pending, Some(first));

        let effects = run(&mut shared, Intent::RefreshCatalogue);
        let second = requested(&effects);
        assert_eq!(effects[0], Effect::Send(Command::Cancel(first)));
        deliver(
            &mut shared,
            Event::Catalogue {
                request: first,
                result: Ok(Catalogue::default()),
            },
        );
        assert!(
            shared.library.catalogue.is_loading(),
            "an old reply is dropped"
        );
        deliver(
            &mut shared,
            Event::Catalogue {
                request: second,
                result: Ok(catalogue(target)),
            },
        );
        assert_eq!(shared.library.revision, 1);
        assert!(shared.library.pending.is_none());
        assert!(shared.library.catalogue.ready().is_some());

        // A refresh keeps the old catalogue on screen until the new one lands.
        let effects = run(&mut shared, Intent::RefreshCatalogue);
        let third = requested(&effects);
        assert!(shared.library.catalogue.ready().is_some());
        deliver(
            &mut shared,
            Event::Catalogue {
                request: third,
                result: Err(Failure::new(FailureKind::FalkorDbDown, "no route")),
            },
        );
        assert!(shared.library.catalogue.failure().is_some());
        assert_eq!(shared.library.revision, 2);
        let effects = run(&mut shared, Intent::RefreshCatalogue);
        let fourth = requested(&effects);
        assert!(
            shared.library.catalogue.is_loading(),
            "a failed catalogue loads again"
        );
        deliver(
            &mut shared,
            Event::Catalogue {
                request: fourth,
                result: Ok(catalogue(target)),
            },
        );

        // A label save: failure is kept for the document, success reloads the catalogue.
        let edit = LabelEdit {
            doc: target,
            add: vec!["options".to_owned()],
            ..LabelEdit::default()
        };
        let effects = run(&mut shared, Intent::SetLabels(edit.clone()));
        let save = requested(&effects);
        assert_eq!(shared.library.busy.get(&target), Some(&Busy::Saving(save)));
        let failure = Failure::internal("disk full");
        let effects = deliver(
            &mut shared,
            Event::LabelsSaved {
                request: save,
                doc: target,
                result: Err(failure.clone()),
            },
        );
        assert!(effects.is_empty() && shared.library.busy.is_empty());
        assert_eq!(shared.library.failures.get(&target), Some(&failure));
        let effects = run(&mut shared, Intent::SetLabels(edit));
        let save = requested(&effects);
        assert!(
            shared.library.failures.is_empty(),
            "a new save clears the old failure"
        );
        let effects = deliver(
            &mut shared,
            Event::LabelsSaved {
                request: save,
                doc: target,
                result: Ok(()),
            },
        );
        assert!(matches!(
            effects.as_slice(),
            [Effect::Send(Command::LoadCatalogue { .. })]
        ));
        let reload = requested(&effects);
        deliver(
            &mut shared,
            Event::Catalogue {
                request: reload,
                result: Ok(catalogue(target)),
            },
        );

        // Nothing is saved or deleted while an ingest runs.
        shared.ingest = IngestJob::Running {
            request: IngestRequest::default(),
            id: RequestId(99),
            progress: None,
        };
        assert!(run(&mut shared, Intent::SetLabels(LabelEdit::default())).is_empty());
        assert!(run(&mut shared, Intent::DeleteDocument(target)).is_empty());
        assert!(shared.library.busy.is_empty());
        shared.ingest = IngestJob::Idle;

        // A failed delete says so and keeps the document.
        let effects = run(&mut shared, Intent::DeleteDocument(target));
        let delete = requested(&effects);
        assert_eq!(
            shared.library.busy.get(&target),
            Some(&Busy::Deleting(delete))
        );
        let refused = Failure::new(FailureKind::QdrantDown, "refused");
        let effects = deliver(
            &mut shared,
            Event::Deleted {
                request: delete,
                doc: target,
                result: Err(refused.clone()),
            },
        );
        assert!(effects.is_empty() && shared.library.busy.is_empty());
        assert_eq!(shared.library.failures.get(&target), Some(&refused));
        assert_eq!(shared.notices[0].kind, NoticeKind::Failed);
        assert_eq!(shared.notices[0].failure.as_ref(), Some(&refused));
        assert_eq!(shared.notices[0].detail, refused.hint);

        // A delete that works reloads the catalogue and closes the source it had open.
        shared.apply_intent(
            Intent::OpenSource {
                doc: target,
                page: 1,
                piece: None,
            },
            &mut Vec::new(),
        );
        let generation = shared.source.generation;
        let pending = shared.source.pending.expect("a page load is on its way");
        let effects = run(&mut shared, Intent::DeleteDocument(target));
        let delete = requested(&effects);
        let effects = deliver(
            &mut shared,
            Event::Deleted {
                request: delete,
                doc: target,
                result: Ok(()),
            },
        );
        assert!(shared.library.busy.is_empty() && shared.library.failures.is_empty());
        assert_eq!(effects[0], Effect::Send(Command::Cancel(pending)));
        assert!(matches!(
            effects.last(),
            Some(Effect::Send(Command::LoadCatalogue { .. }))
        ));
        assert_eq!(shared.source.target, None);
        assert_eq!(shared.source.page, Loadable::Idle);
        assert_eq!(shared.source.generation, generation + 1);
        assert_eq!(shared.notices[0].kind, NoticeKind::Done);
        assert!(shared.notices[0].title.contains("Notes chapter 1"));

        // A reply for another request changes nothing.
        let before = shared.clone();
        deliver(
            &mut shared,
            Event::Deleted {
                request: RequestId(500),
                doc: target,
                result: Ok(()),
            },
        );
        assert_eq!(shared, before);
    }
}
