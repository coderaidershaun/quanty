//! The Datasets page on the whole app and the fake backend: a build starts from the form, shows
//! its stage and its cost while it runs, and ends in a notice; and a build and a delete hold
//! each other off.

mod guards;
mod run;

use contract::{Command, DatasetRequest, DatasetStage};
use gui::state::{DatasetJob, Shared};
use gui::testkit;

use super::Window;
use super::recording::{self, Seen};

/// The stage at which the scenes with a build that never ends stand still.
const STALL: DatasetStage = DatasetStage::WritingSynthetic { done: 6, total: 12 };

fn is_build(command: &Command) -> bool {
    matches!(command, Command::BuildDataset { .. })
}

fn open(scene: &str, size: [f32; 2]) -> (Window, Seen) {
    let (mut harness, seen) = recording::open(scene, size);
    testkit::settle(&mut harness);
    (harness, seen)
}

/// The builds the app sent, in the order it sent them.
fn sent_builds(seen: &Seen) -> Vec<DatasetRequest> {
    seen.all()
        .into_iter()
        .filter_map(|command| match command {
            Command::BuildDataset { build, .. } => Some(build),
            _ => None,
        })
        .collect()
}

/// Whether a build that never ends has played up to its stall.
fn is_at_its_stall(shared: &Shared) -> bool {
    matches!(
        &shared.datasets,
        DatasetJob::Running { progress: Some(progress), .. } if progress.stage == STALL
    )
}
