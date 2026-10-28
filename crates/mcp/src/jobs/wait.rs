//! Waits for a job to change and tells the caller how far it is while it waits. It works for the
//! report of any kind of job, and knows nothing of the server, the stores or the services.

use std::future::Future;
use std::time::Duration;

use rmcp::RoleServer;
use rmcp::model::{ProgressNotificationParam, ProgressToken};
use rmcp::service::Peer;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::watch;
use tokio::time::{MissedTickBehavior, interval, sleep};

use super::view::JobState;
use super::{Job, JobReport};

const MIN_TIMEOUT_S: i64 = 1;
const MAX_TIMEOUT_S: i64 = 90;
const DEFAULT_TIMEOUT_S: i64 = 60;

/// The least time between two progress notifications of one wait.
const PROGRESS_EVERY: Duration = Duration::from_secs(2);

/// The progress bar of a notification runs from 0 to this.
const PROGRESS_TOTAL: f64 = 100.0;

/// The same end in tenths of a percent. The values of a wait are kept in whole tenths so that two
/// of them compare exactly.
const TENTHS_OF_TOTAL: u32 = 1000;

/// What an agent gives `watch_job`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct WatchJobArgs {
    /// The `job_id` that `ingest_pdf`, `write_insights` or `build_dataset` gave.
    pub(crate) job_id: String,
    /// How long to wait for the stage or the state of the job to change, in seconds. 60 when left
    /// out.
    #[serde(default = "default_timeout_s")]
    #[schemars(range(min = MIN_TIMEOUT_S, max = MAX_TIMEOUT_S))]
    timeout_s: i64,
}

fn default_timeout_s() -> i64 {
    DEFAULT_TIMEOUT_S
}

impl WatchJobArgs {
    /// # Errors
    /// [`WatchError::TimeoutOutOfRange`] unless `timeout_s` is from 1 to 90.
    pub(crate) fn timeout(&self) -> Result<Duration, WatchError> {
        let in_range = (MIN_TIMEOUT_S..=MAX_TIMEOUT_S).contains(&self.timeout_s);
        match u64::try_from(self.timeout_s) {
            Ok(seconds) if in_range => Ok(Duration::from_secs(seconds)),
            _ => Err(WatchError::TimeoutOutOfRange {
                timeout_s: self.timeout_s,
            }),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum WatchError {
    #[error(
        "`timeout_s` is {timeout_s}, and it must be from {MIN_TIMEOUT_S} to {MAX_TIMEOUT_S} seconds; leave it out to wait {DEFAULT_TIMEOUT_S} seconds"
    )]
    TimeoutOutOfRange { timeout_s: i64 },
}

/// Where the progress of a wait goes: the client that asked, under the token it gave.
pub(crate) struct ProgressSink {
    peer: Peer<RoleServer>,
    token: ProgressToken,
}

impl ProgressSink {
    pub(crate) fn new(peer: Peer<RoleServer>, token: ProgressToken) -> ProgressSink {
        ProgressSink { peer, token }
    }
}

/// What a wait ends on a change of: the state, and the name of the stage when there is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Phase(JobState, Option<&'static str>);

impl Phase {
    fn of(job: &Job) -> Phase {
        Phase(job.state(), job.stage().map(|stage| stage.name))
    }
}

/// The latest report of the job once its state or the name of its stage differs from what it was
/// when the wait began, or at once when the job is not running, or when `timeout` has passed, or
/// when `cancelled` is ready. A change of the counts or the words inside one stage does not end the
/// wait; it goes to `progress` when the caller gave one.
///
/// Ending the wait never touches the job: the job runs on its own task.
pub(crate) async fn wait_for_change<R: JobReport>(
    mut watcher: watch::Receiver<R>,
    timeout: Duration,
    progress: Option<ProgressSink>,
    cancelled: impl Future<Output = ()>,
) -> R {
    // Marks what the job says now as seen, so that `changed` waits for the next change.
    let at_start = watcher.borrow_and_update().clone();
    if !at_start.is_running() {
        return at_start;
    }
    let began = Phase::of(at_start.job());
    let mut progress = progress.map(ProgressSender::new);
    let mut ticker = interval(PROGRESS_EVERY);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let deadline = sleep(timeout);
    tokio::pin!(deadline, cancelled);
    loop {
        // The branches are tried in order, progress first, so that when the first tick and the
        // deadline are ready at once, the first notification is still sent before the wait ends.
        tokio::select! {
            biased;
            _ = ticker.tick(), if progress.is_some() => {
                if let Some(sender) = progress.as_mut() {
                    let job = watcher.borrow().job().clone();
                    if !sender.send_if_changed(&job).await {
                        progress = None;
                    }
                }
            }
            changed = watcher.changed() => {
                let latest = watcher.borrow_and_update().clone();
                // The error means every writer of the report is gone: it will not change again.
                if changed.is_err() || !latest.is_running() || Phase::of(latest.job()) != began {
                    return latest;
                }
            }
            () = &mut deadline => return watcher.borrow().clone(),
            // A client's cancel does not end this call on its own: it only makes `cancelled`
            // ready, so the wait must end here.
            () = &mut cancelled => return watcher.borrow().clone(),
        }
    }
}

/// The value of the next notification in tenths of a percent, or `None` to send nothing. New words
/// at the same value go one tenth higher, too little to see on a bar, so that a host still shows
/// the new words while the values of one wait only rise. That step never fills the bar: only the
/// work of the job does.
fn next_tenths(last: Option<&(u32, String)>, tenths: u32, message: &str) -> Option<u32> {
    let Some((sent, sent_message)) = last else {
        return Some(tenths);
    };
    if tenths > *sent {
        return Some(tenths);
    }
    if tenths == *sent && message != sent_message {
        let nudged = sent + 1;
        return (nudged < TENTHS_OF_TOTAL).then_some(nudged);
    }
    None
}

/// Sends the progress of a job to the client of a wait: a value that is higher than the last one
/// sent, or the same value with new words.
pub(crate) struct ProgressSender {
    sink: ProgressSink,
    last: Option<(u32, String)>,
}

impl ProgressSender {
    pub(crate) fn new(sink: ProgressSink) -> ProgressSender {
        ProgressSender { sink, last: None }
    }

    /// `false` when the client cannot be reached any more, so the caller sends no more.
    pub(crate) async fn send_if_changed(&mut self, job: &Job) -> bool {
        if job.state() != JobState::Running {
            return true;
        }
        let reached = (job.progress() * f64::from(TENTHS_OF_TOTAL)).round() as u32;
        let message = job.stage().map_or_else(
            || job.state().as_str().to_owned(),
            |stage| stage.words.clone(),
        );
        let Some(tenths) = next_tenths(self.last.as_ref(), reached, &message) else {
            return true;
        };
        let percent = f64::from(tenths) / 10.0;
        let notification = ProgressNotificationParam::new(self.sink.token.clone(), percent)
            .with_total(PROGRESS_TOTAL)
            .with_message(message.clone());
        match self.sink.peer.notify_progress(notification).await {
            Ok(()) => {
                self.last = Some((tenths, message));
                true
            }
            Err(error) => {
                tracing::debug!(%error, "stopped sending progress: the client cannot be reached");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use super::*;
    use crate::jobs::JobKind;

    #[derive(Clone)]
    struct Report(Job);

    impl JobReport for Report {
        fn job(&self) -> &Job {
            &self.0
        }

        fn stopped(&mut self) {
            self.0.stop("stopped".to_owned());
        }
    }

    #[test]
    fn a_notification_rises_and_a_change_of_words_alone_moves_it_up_by_a_tenth() {
        let sent = |tenths: u32, words: &str| Some((tenths, words.to_owned()));

        assert_eq!(next_tenths(None, 0, "first"), Some(0));
        assert_eq!(next_tenths(sent(120, "a").as_ref(), 630, "a"), Some(630));
        assert_eq!(next_tenths(sent(730, "a").as_ref(), 730, "b"), Some(731));
        assert_eq!(next_tenths(sent(730, "a").as_ref(), 730, "a"), None);
        assert_eq!(next_tenths(sent(731, "b").as_ref(), 730, "c"), None);
        assert_eq!(next_tenths(sent(731, "b").as_ref(), 731, "b"), None);
        assert_eq!(next_tenths(sent(999, "a").as_ref(), 999, "b"), None);
        assert_eq!(next_tenths(sent(990, "a").as_ref(), 990, "b"), Some(991));
    }

    #[tokio::test]
    async fn a_cancelled_wait_ends_and_the_job_goes_on() {
        let (report, watcher) =
            watch::channel(Report(Job::running(Uuid::new_v4(), JobKind::Ingest)));
        let (cancel, cancelled) = oneshot::channel::<()>();
        let waiting = tokio::spawn(wait_for_change(
            watcher,
            Duration::from_secs(30),
            None,
            async {
                let _ = cancelled.await;
            },
        ));
        tokio::task::yield_now().await;

        cancel.send(()).unwrap();

        let latest = tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("a cancel should end the wait")
            .unwrap();
        assert_eq!(latest.job().state(), JobState::Running);
        assert_eq!(report.borrow().job().state(), JobState::Running);
    }
}
