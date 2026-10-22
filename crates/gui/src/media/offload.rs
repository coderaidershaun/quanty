//! Runs queued picture and formula jobs between frames, within the time the caller allows.

use std::collections::VecDeque;
use std::time::Duration;

use eframe::egui;
use web_time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offload {
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Now,
    Later,
}

pub struct Workers<Job, Done> {
    jobs: VecDeque<Job>,
    done: VecDeque<Done>,
    ctx: egui::Context,
    work: fn(&egui::Context, Job) -> Done,
}

impl<Job, Done> Workers<Job, Done> {
    /// `work` must catch any panic from the library it calls.
    pub fn start(
        _offload: Offload,
        ctx: egui::Context,
        work: fn(&egui::Context, Job) -> Done,
    ) -> Self {
        Workers {
            jobs: VecDeque::new(),
            done: VecDeque::new(),
            ctx,
            work,
        }
    }

    pub fn submit(&mut self, job: Job, urgency: Urgency) {
        match urgency {
            Urgency::Now => self.jobs.push_front(job),
            Urgency::Later => self.jobs.push_back(job),
        }
        self.ctx.request_repaint();
    }

    pub fn cancel(&mut self, mut unwanted: impl FnMut(&Job) -> bool) -> usize {
        let before = self.jobs.len();
        self.jobs.retain(|job| !unwanted(job));
        before - self.jobs.len()
    }

    pub fn promote(&mut self, wanted: impl Fn(&Job) -> bool) {
        if let Some(index) = self.jobs.iter().position(wanted)
            && let Some(job) = self.jobs.remove(index)
        {
            self.jobs.push_front(job);
        }
    }

    pub fn take_done(&mut self) -> Option<Done> {
        self.done.pop_front()
    }

    pub fn run_pending(&mut self) {
        while let Some(job) = self.jobs.pop_front() {
            self.done.push_back((self.work)(&self.ctx, job));
        }
        if !self.done.is_empty() {
            self.ctx.request_repaint();
        }
    }

    /// A job already started completes before the time budget is checked again.
    pub fn run_for(&mut self, duration: Duration) {
        let started = Instant::now();
        while started.elapsed() < duration {
            let Some(job) = self.jobs.pop_front() else {
                break;
            };
            self.done.push_back((self.work)(&self.ctx, job));
        }
        if !self.jobs.is_empty() || !self.done.is_empty() {
            self.ctx.request_repaint();
        }
    }
}
