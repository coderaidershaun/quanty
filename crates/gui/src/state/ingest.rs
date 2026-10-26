//! The one ingest the person can have at a time: checked, started, watched, finished.

use super::shared::{Shared, push_cancel};
use contract::{
    Command, Effect, Failure, IngestOutcome, IngestProgress, IngestRequest, NoticeKind, Preflight,
    RequestId, Usage,
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
    Finished {
        request: IngestRequest,
        /// Latest cumulative spend, retained even when the required stage fails.
        spent: Usage,
        result: Result<IngestOutcome, Failure>,
    },
}

impl IngestJob {
    pub fn is_running(&self) -> bool {
        matches!(self, IngestJob::Running { .. })
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
        if !self.health.facts.running.is_empty() {
            return;
        }
        // The backend keeps a delete and an ingest apart only while the pages are converted, so no
        // ingest starts while a delete is on its way.
        if self.library.is_deleting() {
            return;
        }
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
            IngestJob::Running { id, progress, .. } if *id == request => {
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
            IngestJob::Running { id, .. } if *id == request => {}
            _ => return,
        }
        let IngestJob::Running {
            request, progress, ..
        } = std::mem::take(&mut self.ingest)
        else {
            return;
        };
        self.announce_ingest(&request, &result);
        if matches!(result, Ok(IngestOutcome::Ingested(_))) {
            self.refresh_catalogue(effects);
        }
        let spent = progress.map_or_else(Usage::default, |progress| progress.spent);
        self.ingest = IngestJob::Finished {
            request,
            result,
            spent,
        };
    }

    fn announce_ingest(
        &mut self,
        request: &IngestRequest,
        result: &Result<IngestOutcome, Failure>,
    ) {
        let file = request.file_name.clone();
        match result {
            Ok(IngestOutcome::Ingested(report)) => {
                let mut detail = format!(
                    "{} pages with {}. {} concepts created, {} linked.",
                    report.pages, report.items, report.concepts_created, report.concepts_linked
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
                );
            }
            Ok(IngestOutcome::Cancelled) => {}
            Err(failure) => {
                self.fail_notice(format!("Ingest of {file} failed"), failure);
            }
        }
    }
}
