//! Runs small jobs for the picture and formula caches: on a few worker threads in the app, and
//! only when a test says so under a test.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offload {
    Threads,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    /// Goes to the front of the queue.
    Now,
    /// Goes to the back of the queue.
    Later,
}

struct Queue<Job> {
    jobs: Mutex<VecDeque<Job>>,
    arrived: Condvar,
    closed: AtomicBool,
}

impl<Job> Queue<Job> {
    /// A worker that panicked cannot leave the list half changed, so a poisoned lock is safe.
    fn lock(&self) -> MutexGuard<'_, VecDeque<Job>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What `run_pending` needs in manual mode.
struct ByHand<Job, Done> {
    ctx: egui::Context,
    work: fn(&egui::Context, Job) -> Done,
    sender: Sender<Done>,
}

/// A queue of jobs and the results that come back. In `Threads` mode a few named threads take
/// the jobs; in `Manual` mode `run_pending` does it on the caller's thread.
pub struct Workers<Job, Done> {
    queue: Arc<Queue<Job>>,
    done: Receiver<Done>,
    by_hand: Option<ByHand<Job, Done>>,
}

impl<Job: Send + 'static, Done: Send + 'static> Workers<Job, Done> {
    /// `work` must not panic: each user catches the panics of the library it calls.
    pub fn start(
        offload: Offload,
        name: &'static str,
        threads: usize,
        ctx: egui::Context,
        work: fn(&egui::Context, Job) -> Done,
    ) -> Self {
        let queue = Arc::new(Queue {
            jobs: Mutex::new(VecDeque::new()),
            arrived: Condvar::new(),
            closed: AtomicBool::new(false),
        });
        let (sender, done) = channel();
        if offload == Offload::Manual {
            let by_hand = Some(ByHand { ctx, work, sender });
            return Workers {
                queue,
                done,
                by_hand,
            };
        }
        for number in 0..threads {
            let (queue, ctx, sender) = (Arc::clone(&queue), ctx.clone(), sender.clone());
            std::thread::Builder::new()
                .name(format!("{name}-{number}"))
                .spawn(move || serve(&queue, &ctx, work, &sender))
                .expect("the operating system starts a worker thread");
        }
        Workers {
            queue,
            done,
            by_hand: None,
        }
    }

    /// `Now` goes to the front of the queue, `Later` to the back.
    pub fn submit(&self, job: Job, urgency: Urgency) {
        let mut jobs = self.queue.lock();
        match urgency {
            Urgency::Now => jobs.push_front(job),
            Urgency::Later => jobs.push_back(job),
        }
        self.queue.arrived.notify_one();
    }

    /// Removes the queued jobs that `unwanted` picks, and says how many. A job that is already
    /// running is not touched.
    pub fn cancel(&self, mut unwanted: impl FnMut(&Job) -> bool) -> usize {
        let mut jobs = self.queue.lock();
        let before = jobs.len();
        jobs.retain(|job| !unwanted(job));
        before - jobs.len()
    }

    /// One finished result, if there is one.
    pub fn take_done(&mut self) -> Option<Done> {
        self.done.try_recv().ok()
    }

    /// Manual only: runs every queued job here, front first.
    pub fn run_pending(&mut self) {
        let Some(by_hand) = &self.by_hand else {
            return;
        };
        let mut ran = false;
        loop {
            let next = self.queue.lock().pop_front();
            let Some(job) = next else {
                break;
            };
            if by_hand
                .sender
                .send((by_hand.work)(&by_hand.ctx, job))
                .is_err()
            {
                return;
            }
            ran = true;
        }
        if ran {
            by_hand.ctx.request_repaint();
        }
    }
}

impl<Job, Done> Drop for Workers<Job, Done> {
    fn drop(&mut self) {
        self.queue.closed.store(true, Ordering::SeqCst);
        self.queue.arrived.notify_all();
    }
}

/// The loop of one worker thread.
fn serve<Job, Done>(
    queue: &Queue<Job>,
    ctx: &egui::Context,
    work: fn(&egui::Context, Job) -> Done,
    sender: &Sender<Done>,
) {
    loop {
        let job = {
            let mut jobs = queue.lock();
            loop {
                if queue.closed.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(job) = jobs.pop_front() {
                    break job;
                }
                jobs = queue
                    .arrived
                    .wait(jobs)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        if sender.send(work(ctx, job)).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn double(_ctx: &egui::Context, job: u32) -> u32 {
        job * 2
    }

    #[test]
    fn a_queued_job_runs_once_in_order_and_a_cancelled_job_never_runs() {
        let ctx = egui::Context::default();
        let mut manual = Workers::start(Offload::Manual, "test", 1, ctx.clone(), double);
        manual.submit(1, Urgency::Later);
        manual.submit(2, Urgency::Later);
        manual.submit(3, Urgency::Now);
        manual.submit(4, Urgency::Later);
        assert!(
            manual.take_done().is_none(),
            "nothing runs before run_pending"
        );

        assert_eq!(manual.cancel(|job| *job == 4), 1);
        manual.run_pending();
        let finished: Vec<u32> = std::iter::from_fn(|| manual.take_done()).collect();
        assert_eq!(
            finished,
            vec![6, 2, 4],
            "the urgent job first, then in order, and no 4"
        );
        manual.run_pending();
        assert!(manual.take_done().is_none(), "a job runs once");

        let mut threads = Workers::start(Offload::Threads, "test", 1, ctx, double);
        threads.submit(5, Urgency::Later);
        let started = Instant::now();
        let result = loop {
            if let Some(done) = threads.take_done() {
                break Some(done);
            }
            if started.elapsed() > Duration::from_secs(5) {
                break None;
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(result, Some(10), "a thread delivers with no run_pending");
    }
}
