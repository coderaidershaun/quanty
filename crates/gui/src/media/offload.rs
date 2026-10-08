//! Runs small jobs for the picture and formula caches: on a few worker threads in the app, and in
//! `Manual` mode, which tests use, only when the caller says so.

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

/// In `Threads` mode a few named threads take the jobs; in `Manual` mode `run_pending` does it on
/// the caller's thread.
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

    pub fn submit(&self, job: Job, urgency: Urgency) {
        let mut jobs = self.queue.lock();
        match urgency {
            Urgency::Now => jobs.push_front(job),
            Urgency::Later => jobs.push_back(job),
        }
        self.queue.arrived.notify_one();
    }

    /// A job that is already running is not touched.
    pub fn cancel(&self, mut unwanted: impl FnMut(&Job) -> bool) -> usize {
        let mut jobs = self.queue.lock();
        let before = jobs.len();
        jobs.retain(|job| !unwanted(job));
        before - jobs.len()
    }

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
