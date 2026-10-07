//! The one ingest the person can have at a time: checked, started, watched, stopped, finished.

use super::Quit;
use super::shared::{Shared, counts_text, push_cancel};
use crate::contract::{
    Command, Effect, Failure, IngestOutcome, IngestProgress, IngestRequest, NoticeKind, Preflight,
    RequestId,
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub enum IngestJob {
    #[default]
    Idle,
    Checking {
        request: IngestRequest,
        id: RequestId,
    },
    Checked {
        request: IngestRequest,
        preflight: Preflight,
    },
    CheckFailed {
        request: IngestRequest,
        failure: Failure,
    },
    Running {
        request: IngestRequest,
        id: RequestId,
        progress: Option<IngestProgress>,
    },
    /// Cancel was asked for. The job ends at its next safe point.
    Stopping {
        request: IngestRequest,
        id: RequestId,
        progress: Option<IngestProgress>,
    },
    Finished {
        request: IngestRequest,
        result: Result<IngestOutcome, Failure>,
    },
}

impl IngestJob {
    /// True while `Running` or `Stopping`.
    pub fn is_running(&self) -> bool {
        matches!(self, IngestJob::Running { .. } | IngestJob::Stopping { .. })
    }

    pub(super) fn is_working(&self) -> bool {
        self.is_running() || matches!(self, IngestJob::Checking { .. })
    }
}

impl Shared {
    pub(super) fn check_ingest(&mut self, request: IngestRequest, effects: &mut Vec<Effect>) {
        if self.ingest.is_running() {
            return;
        }
        let id = self.issue_request();
        if let IngestJob::Checking { id: old, .. } = &self.ingest {
            push_cancel(effects, *old);
        }
        self.ingest = IngestJob::Checking {
            request: request.clone(),
            id,
        };
        effects.push(Effect::Send(Command::Preflight {
            request: id,
            ingest: request,
        }));
    }

    pub(super) fn start_ingest(&mut self, effects: &mut Vec<Effect>) {
        let IngestJob::Checked { request, preflight } = &self.ingest else {
            return;
        };
        if !preflight.blockers.is_empty() {
            return;
        }
        let request = request.clone();
        let id = self.issue_request();
        self.ingest = IngestJob::Running {
            request: request.clone(),
            id,
            progress: None,
        };
        effects.push(Effect::Send(Command::Ingest {
            request: id,
            ingest: request,
        }));
    }

    /// Asks a running ingest to stop. It keeps running until it reaches a safe point.
    pub(super) fn cancel_ingest(&mut self, effects: &mut Vec<Effect>) {
        self.ingest = match std::mem::take(&mut self.ingest) {
            IngestJob::Running {
                request,
                id,
                progress,
            } => {
                push_cancel(effects, id);
                IngestJob::Stopping {
                    request,
                    id,
                    progress,
                }
            }
            other => other,
        };
    }

    pub(super) fn clear_ingest(&mut self, effects: &mut Vec<Effect>) {
        if self.ingest.is_running() {
            return;
        }
        if let IngestJob::Checking { id, .. } = &self.ingest {
            push_cancel(effects, *id);
        }
        self.ingest = IngestJob::Idle;
    }

    pub(super) fn preflight_arrived(
        &mut self,
        request: RequestId,
        result: Result<Preflight, Failure>,
    ) {
        match &self.ingest {
            IngestJob::Checking { id, .. } if *id == request => {}
            _ => return,
        }
        if let Err(failure) = &result {
            self.mark_down(failure);
        }
        let IngestJob::Checking { request, .. } = std::mem::take(&mut self.ingest) else {
            return;
        };
        self.ingest = match result {
            Ok(preflight) => IngestJob::Checked { request, preflight },
            Err(failure) => IngestJob::CheckFailed { request, failure },
        };
    }

    pub(super) fn progress_arrived(&mut self, request: RequestId, newest: IngestProgress) {
        match &mut self.ingest {
            IngestJob::Running { id, progress, .. } | IngestJob::Stopping { id, progress, .. }
                if *id == request =>
            {
                *progress = Some(newest);
            }
            _ => {}
        }
    }

    pub(super) fn ingest_finished(
        &mut self,
        request: RequestId,
        result: Result<IngestOutcome, Failure>,
        effects: &mut Vec<Effect>,
    ) {
        match &self.ingest {
            IngestJob::Running { id, .. } | IngestJob::Stopping { id, .. } if *id == request => {}
            _ => return,
        }
        let (IngestJob::Running { request, .. } | IngestJob::Stopping { request, .. }) =
            std::mem::take(&mut self.ingest)
        else {
            return;
        };
        self.announce_ingest(&request, &result);
        if matches!(result, Ok(IngestOutcome::Ingested(_))) {
            self.refresh_catalogue(effects);
        }
        if self.quit == Quit::Confirmed {
            effects.push(Effect::CloseWindow);
        }
        self.ingest = IngestJob::Finished { request, result };
    }

    /// Leaves a notice for how the ingest ended. A cancelled run leaves none.
    fn announce_ingest(
        &mut self,
        request: &IngestRequest,
        result: &Result<IngestOutcome, Failure>,
    ) {
        let file = request.pdf.file_name().map_or_else(
            || request.pdf.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        match result {
            Ok(IngestOutcome::Ingested(report)) => {
                let mut detail = format!(
                    "{} pages with {}. {} concepts created, {} linked.",
                    report.pages,
                    counts_text(&report.items),
                    report.concepts_created,
                    report.concepts_linked
                );
                if !report.pages_to_check.is_empty() {
                    detail.push_str(&format!(
                        " {} pages need a check.",
                        report.pages_to_check.len()
                    ));
                }
                self.add_notice(
                    NoticeKind::Done,
                    format!("Ingested {}", report.title),
                    detail,
                    None,
                );
            }
            Ok(IngestOutcome::AlreadyIngested { doc, items, .. }) => {
                let name = self
                    .library
                    .catalogue
                    .ready()
                    .and_then(|catalogue| catalogue.document(*doc))
                    .map_or(file, |document| document.title.clone());
                self.add_notice(
                    NoticeKind::Done,
                    format!("{name} was already ingested"),
                    format!("{items} items are stored. Nothing was converted or embedded."),
                    None,
                );
            }
            Ok(IngestOutcome::Cancelled) => {}
            Err(failure) => {
                self.mark_down(failure);
                self.add_notice(
                    NoticeKind::Failed,
                    format!("Ingest of {file} failed"),
                    failure.hint.clone(),
                    Some(failure.clone()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::*;
    use crate::contract::{
        Command, DocId, Event, FailureKind, IngestStage, Intent, NoticeKind, Service, ServiceState,
    };
    use crate::state::Quit;
    use crate::testkit::sample;

    fn request() -> IngestRequest {
        IngestRequest {
            pdf: PathBuf::from("/books/chapter-1-intro.pdf"),
            book: "Notes".to_owned(),
            ..IngestRequest::default()
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

    fn preflight_id(effects: &[Effect]) -> RequestId {
        match effects.last() {
            Some(Effect::Send(Command::Preflight { request, .. })) => *request,
            other => panic!("expected a preflight, got {other:?}"),
        }
    }

    fn checked(blockers: Vec<Failure>) -> Preflight {
        Preflight {
            blockers,
            ..sample::preflight()
        }
    }

    fn checked_ok(shared: &mut Shared, blockers: Vec<Failure>) {
        let effects = run(shared, Intent::CheckIngest(request()));
        let id = preflight_id(&effects);
        deliver(
            shared,
            Event::Preflight {
                request: id,
                result: Ok(checked(blockers)),
            },
        );
    }

    fn started(shared: &mut Shared) -> RequestId {
        checked_ok(shared, Vec::new());
        match run(shared, Intent::StartIngest).as_slice() {
            [Effect::Send(Command::Ingest { request, .. })] => *request,
            other => panic!("expected an ingest, got {other:?}"),
        }
    }

    #[test]
    fn one_ingest_goes_from_check_to_notice_and_nothing_starts_while_it_runs_or_stops() {
        let mut shared = Shared::default();
        assert!(
            run(&mut shared, Intent::StartIngest).is_empty(),
            "nothing is checked yet"
        );

        // A check can be asked again; the old answer is dropped.
        let first = preflight_id(&run(&mut shared, Intent::CheckIngest(request())));
        let effects = run(&mut shared, Intent::CheckIngest(request()));
        let second = preflight_id(&effects);
        assert_eq!(effects[0], Effect::Send(Command::Cancel(first)));
        deliver(
            &mut shared,
            Event::Preflight {
                request: first,
                result: Ok(checked(Vec::new())),
            },
        );
        assert!(matches!(shared.ingest, IngestJob::Checking { .. }));

        // A failed check names the service it found down.
        let missing = Failure::new(FailureKind::PopplerMissing, "pdftoppm not found");
        deliver(
            &mut shared,
            Event::Preflight {
                request: second,
                result: Err(missing.clone()),
            },
        );
        assert!(
            matches!(&shared.ingest, IngestJob::CheckFailed { failure, .. } if *failure == missing)
        );
        assert!(matches!(
            shared.health.of(Service::Poppler),
            ServiceState::Down(_)
        ));

        // A blocker keeps a paid run from starting.
        checked_ok(&mut shared, vec![missing]);
        assert!(run(&mut shared, Intent::StartIngest).is_empty());
        assert!(matches!(shared.ingest, IngestJob::Checked { .. }));

        // A clean check starts one run.
        let id = started(&mut shared);
        assert!(
            matches!(&shared.ingest, IngestJob::Running { id: running, progress: None, .. } if *running == id)
        );
        assert!(run(&mut shared, Intent::CheckIngest(request())).is_empty());
        assert!(run(&mut shared, Intent::ClearIngest).is_empty());
        assert!(run(&mut shared, Intent::StartIngest).is_empty());

        let step = |done| IngestProgress {
            stage: IngestStage::Converting,
            done: Some(done),
            total: Some(7),
            pages_failed: 0,
            cost_usd: 0.1,
        };
        deliver(
            &mut shared,
            Event::IngestProgress {
                request: RequestId(900),
                progress: step(1),
            },
        );
        deliver(
            &mut shared,
            Event::IngestProgress {
                request: id,
                progress: step(2),
            },
        );
        deliver(
            &mut shared,
            Event::IngestProgress {
                request: id,
                progress: step(3),
            },
        );
        assert!(
            matches!(&shared.ingest, IngestJob::Running { progress: Some(p), .. } if p.done == Some(3))
        );

        // Stopping keeps the same id and progress, and blocks every start.
        let effects = run(&mut shared, Intent::CancelIngest);
        assert_eq!(effects, vec![Effect::Send(Command::Cancel(id))]);
        assert!(
            matches!(&shared.ingest, IngestJob::Stopping { id: stopping, progress: Some(_), .. } if *stopping == id)
        );
        assert!(shared.ingest.is_running());
        assert!(run(&mut shared, Intent::CancelIngest).is_empty());
        assert!(run(&mut shared, Intent::CheckIngest(request())).is_empty());
        assert!(run(&mut shared, Intent::StartIngest).is_empty());
        let before = shared.clone();
        deliver(
            &mut shared,
            Event::IngestFinished {
                request: RequestId(900),
                result: Ok(IngestOutcome::Cancelled),
            },
        );
        assert_eq!(shared, before, "a reply of another request changes nothing");

        // A job that was stopping can still end as a finished ingest.
        let effects = deliver(
            &mut shared,
            Event::IngestFinished {
                request: id,
                result: Ok(IngestOutcome::Ingested(sample::ingest_report())),
            },
        );
        assert!(matches!(
            shared.ingest,
            IngestJob::Finished {
                result: Ok(IngestOutcome::Ingested(_)),
                ..
            }
        ));
        assert!(matches!(
            effects.as_slice(),
            [Effect::Send(Command::LoadCatalogue { .. })]
        ));
        assert_eq!(shared.notices[0].kind, NoticeKind::Done);
        assert!(shared.notices[0].title.contains("Option Volatility"));
        assert!(shared.notices[0].detail.contains("7 pages"));
        assert!(!shared.ingest.is_running());

        // A cancelled run leaves no notice.
        run(&mut shared, Intent::ClearIngest);
        assert_eq!(shared.ingest, IngestJob::Idle);
        let id = started(&mut shared);
        run(&mut shared, Intent::CancelIngest);
        let notices = shared.notices.len();
        deliver(
            &mut shared,
            Event::IngestFinished {
                request: id,
                result: Ok(IngestOutcome::Cancelled),
            },
        );
        assert_eq!(shared.notices.len(), notices);
        assert!(matches!(
            shared.ingest,
            IngestJob::Finished {
                result: Ok(IngestOutcome::Cancelled),
                ..
            }
        ));

        // A run that was already ingested says so and reloads nothing.
        run(&mut shared, Intent::ClearIngest);
        let id = started(&mut shared);
        let effects = deliver(
            &mut shared,
            Event::IngestFinished {
                request: id,
                result: Ok(IngestOutcome::AlreadyIngested {
                    doc: DocId(Uuid::from_u128(1)),
                    items: 12,
                    pages_to_check: None,
                }),
            },
        );
        assert!(effects.is_empty());
        assert_eq!(shared.notices[0].kind, NoticeKind::Done);
        assert!(shared.notices[0].title.contains("chapter-1-intro.pdf"));

        // A failed run carries its failure and its hint.
        run(&mut shared, Intent::ClearIngest);
        let id = started(&mut shared);
        let failure = Failure::new(FailureKind::PageFailed, "page 4");
        deliver(
            &mut shared,
            Event::IngestFinished {
                request: id,
                result: Err(failure.clone()),
            },
        );
        assert_eq!(shared.notices[0].kind, NoticeKind::Failed);
        assert_eq!(shared.notices[0].failure.as_ref(), Some(&failure));
        assert_eq!(shared.notices[0].detail, failure.hint);

        // Clearing a check that is still running stops it.
        run(&mut shared, Intent::ClearIngest);
        let check = preflight_id(&run(&mut shared, Intent::CheckIngest(request())));
        let effects = run(&mut shared, Intent::ClearIngest);
        assert_eq!(effects, vec![Effect::Send(Command::Cancel(check))]);
        assert_eq!(shared.ingest, IngestJob::Idle);
        assert_eq!(shared.quit, Quit::No);
    }
}
