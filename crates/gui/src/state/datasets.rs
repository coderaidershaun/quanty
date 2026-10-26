//! The one dataset build the person can have at a time: started, watched, finished.

use super::shared::Shared;
use contract::{
    Command, DatasetProgress, DatasetReport, DatasetRequest, Effect, Failure, NoticeKind, RequestId,
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]
pub enum DatasetJob {
    #[default]
    Idle,
    Running {
        request: DatasetRequest,
        id: RequestId,
        progress: Option<DatasetProgress>,
    },
    Finished {
        request: DatasetRequest,
        result: Result<DatasetReport, Failure>,
    },
}

impl DatasetJob {
    pub fn is_running(&self) -> bool {
        matches!(self, DatasetJob::Running { .. })
    }

    /// The request of a running or finished build.
    pub fn request(&self) -> Option<&DatasetRequest> {
        match self {
            DatasetJob::Idle => None,
            DatasetJob::Running { request, .. } | DatasetJob::Finished { request, .. } => {
                Some(request)
            }
        }
    }
}

impl Shared {
    /// A build reads the converted documents that a delete removes, so none starts while a delete
    /// is on its way.
    pub fn can_start_dataset_build(&self) -> bool {
        !self.datasets.is_running()
            && !self.library.is_deleting()
            && self.health.facts.running.is_empty()
    }

    pub(super) fn start_dataset_build(
        &mut self,
        request: DatasetRequest,
        effects: &mut Vec<Effect>,
    ) {
        if !self.can_start_dataset_build() {
            return;
        }
        let id = self.issue_request();
        self.datasets = DatasetJob::Running {
            request: request.clone(),
            id,
            progress: None,
        };
        effects.push(Effect::Send(Command::BuildDataset {
            request: id,
            build: request,
        }));
    }

    pub(super) fn clear_dataset_build(&mut self) {
        if !self.datasets.is_running() {
            self.datasets = DatasetJob::Idle;
        }
    }

    pub(super) fn dataset_progress_arrived(&mut self, request: RequestId, newest: DatasetProgress) {
        if let DatasetJob::Running { id, progress, .. } = &mut self.datasets
            && *id == request
        {
            *progress = Some(newest);
        }
    }

    pub(super) fn dataset_build_finished(
        &mut self,
        request: RequestId,
        result: Result<DatasetReport, Failure>,
    ) {
        match &self.datasets {
            DatasetJob::Running { id, .. } if *id == request => {}
            _ => return,
        }
        let DatasetJob::Running { request, .. } = std::mem::take(&mut self.datasets) else {
            return;
        };
        self.announce_dataset_build(&request, &result);
        self.datasets = DatasetJob::Finished { request, result };
    }

    fn announce_dataset_build(
        &mut self,
        request: &DatasetRequest,
        result: &Result<DatasetReport, Failure>,
    ) {
        match result {
            Ok(report) => {
                self.add_notice(
                    NoticeKind::Done,
                    format!("Built {}", report.name),
                    format!("{} Saved in {}.", report.summary(), report.folder.display()),
                );
            }
            Err(failure) => {
                self.fail_notice(format!("Could not build {}", request.name), failure);
            }
        }
    }
}
