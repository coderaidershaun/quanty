//! The formulas that were asked for and what became of each: queued, typeset on a worker thread,
//! and kept as textures within a byte budget.

use std::collections::HashMap;
use std::error::Error as _;
use std::panic::{AssertUnwindSafe, catch_unwind};

use eframe::egui;

use super::image::{Display, MathImage, MathRef, MathState};
use super::typeset::{TypesetError, typeset};
use crate::media::{Offload, Urgency, Workers};

const DEFAULT_BUDGET: usize = 64 * 1024 * 1024;
const WORKER_NAME: &str = "quanty-math";
const TYPESET_THREADS: usize = 1;
const BYTES_PER_PIXEL: usize = 4;
/// Below this a formula cannot be read.
const MIN_EM_PX: f32 = 4.0;
/// Above this the picture costs more than it can show.
const MAX_EM_PX: f32 = 256.0;

/// The colour is not part of it: ink is white and `paint` tints it.
struct Job {
    key: u64,
    latex: Box<str>,
    display: Display,
    em_px: u16,
    pixels_per_point: f32,
    max_side: u32,
}

/// `made` is `None` when the formula cannot be typeset.
struct Done {
    job: Job,
    made: Option<Made>,
}

/// Dropping it frees the texture.
struct Made {
    _texture: egui::TextureHandle,
    image: MathImage,
    bytes: usize,
}

enum Slot {
    Loading,
    Ready(Made),
    /// Kept, so that a formula that cannot be typeset is not tried again in every frame. It
    /// holds no texture.
    // SMELL: a failed formula is never dropped, and its entry keeps the whole source. Memory
    // grows with every different formula that fails, because the byte budget counts textures
    // only.
    Failed,
}

struct Entry {
    latex: Box<str>,
    display: Display,
    em_px: u16,
    /// The number of polls so far when a caller last asked for this formula.
    last_used: u64,
    slot: Slot,
}

impl Entry {
    fn is_for(&self, latex: &str, display: Display, em_px: u16) -> bool {
        *self.latex == *latex && self.display == display && self.em_px == em_px
    }

    fn state(&self) -> MathState {
        match &self.slot {
            Slot::Loading => MathState::Loading,
            Slot::Ready(made) => MathState::Ready(made.image),
            Slot::Failed => MathState::Failed,
        }
    }

    fn is_loading(&self) -> bool {
        matches!(self.slot, Slot::Loading)
    }
}

pub struct Math {
    workers: Workers<Job, Done>,
    /// Keyed by a hash of the source, the mode and the pixels to the em, so that a lookup by
    /// `&str` allocates nothing. The entry keeps what it was made for, to catch a shared hash.
    entries: HashMap<u64, Entry>,
    budget: usize,
    /// The bytes of every texture in `entries`.
    bytes: usize,
    /// A formula is wanted when it was asked for since the last poll.
    polls: u64,
    pixels_per_point: f32,
    max_side: u32,
}

impl Math {
    pub fn new(ctx: &egui::Context, offload: Offload) -> Math {
        Math {
            workers: Workers::start(offload, WORKER_NAME, TYPESET_THREADS, ctx.clone(), work),
            entries: HashMap::new(),
            budget: DEFAULT_BUDGET,
            bytes: 0,
            polls: 0,
            pixels_per_point: ctx.pixels_per_point(),
            max_side: max_side(ctx),
        }
    }

    /// A formula that is asked for in every frame stays, even beyond the budget.
    pub fn with_budget(mut self, bytes: usize) -> Math {
        self.budget = bytes;
        self
    }

    /// Call this once per frame, before any panel draws.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let last_drawn = self.polls;
        self.polls += 1;
        self.pixels_per_point = ctx.pixels_per_point();
        self.max_side = max_side(ctx);
        while let Some(done) = self.workers.take_done() {
            self.store(done);
        }
        self.cancel_unwanted(last_drawn);
        self.evict(last_drawn);
    }

    /// With `Offload::Manual`: typesets every queued formula on the calling thread.
    pub fn run_pending(&mut self) {
        self.workers.run_pending();
    }

    pub fn is_idle(&self) -> bool {
        !self.entries.values().any(Entry::is_loading)
    }

    /// It never blocks.
    pub fn get(&mut self, math: &MathRef<'_>) -> MathState {
        let em_px = em_px(math.size, self.pixels_per_point);
        let key = egui::util::hash((math.latex, math.display, em_px));
        match self.entries.get_mut(&key) {
            Some(entry) if entry.is_for(math.latex, math.display, em_px) => {
                entry.last_used = self.polls;
                return entry.state();
            }
            Some(_) => self.remove(key),
            None => {}
        }
        self.entries.insert(
            key,
            Entry {
                latex: math.latex.into(),
                display: math.display,
                em_px,
                last_used: self.polls,
                slot: Slot::Loading,
            },
        );
        self.workers.submit(
            Job {
                key,
                latex: math.latex.into(),
                display: math.display,
                em_px,
                pixels_per_point: self.pixels_per_point,
                max_side: self.max_side,
            },
            Urgency::Later,
        );
        MathState::Loading
    }

    fn store(&mut self, done: Done) {
        let Done { job, made } = done;
        let Some(entry) = self.entries.get_mut(&job.key) else {
            return;
        };
        // Two sources can share a hash. A result for the one that was replaced is dropped.
        if !entry.is_loading() || !entry.is_for(&job.latex, job.display, job.em_px) {
            return;
        }
        entry.slot = match made {
            Some(made) => {
                self.bytes += made.bytes;
                Slot::Ready(made)
            }
            None => Slot::Failed,
        };
    }

    /// A job that already runs is left alone: its entry stays until the result arrives.
    fn cancel_unwanted(&mut self, last_drawn: u64) {
        let unwanted: Vec<u64> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_loading() && entry.last_used < last_drawn)
            .map(|(key, _)| *key)
            .collect();
        if unwanted.is_empty() {
            return;
        }
        let mut cancelled = Vec::new();
        self.workers.cancel(|job| {
            let hit = unwanted.contains(&job.key);
            if hit {
                cancelled.push(job.key);
            }
            hit
        });
        for key in &cancelled {
            self.entries.remove(key);
        }
        if !cancelled.is_empty() {
            tracing::debug!(
                cancelled = cancelled.len(),
                "formulas nobody asked for were not typeset"
            );
        }
    }

    /// A texture asked for since the last poll is on screen and stays.
    fn evict(&mut self, last_drawn: u64) {
        if self.bytes <= self.budget {
            return;
        }
        let mut idle: Vec<(u64, u64)> = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                matches!(entry.slot, Slot::Ready(_)) && entry.last_used < last_drawn
            })
            .map(|(key, entry)| (entry.last_used, *key))
            .collect();
        idle.sort_unstable();
        for (_, key) in idle {
            if self.bytes <= self.budget {
                break;
            }
            self.remove(key);
        }
    }

    fn remove(&mut self, key: u64) {
        if let Some(Entry {
            slot: Slot::Ready(made),
            ..
        }) = self.entries.remove(&key)
        {
            self.bytes -= made.bytes;
        }
    }
}

fn max_side(ctx: &egui::Context) -> u32 {
    u32::try_from(ctx.input(|input| input.max_texture_side)).unwrap_or(u32::MAX)
}

/// Pixels to the em for a font size in points.
fn em_px(size: f32, pixels_per_point: f32) -> u16 {
    (size * pixels_per_point)
        .round()
        .clamp(MIN_EM_PX, MAX_EM_PX) as u16
}

/// The job of a worker thread. No panic leaves it: a formula that panics the typesetting library
/// is a formula that fails.
fn work(ctx: &egui::Context, job: Job) -> Done {
    let result =
        catch_unwind(AssertUnwindSafe(|| make(ctx, &job))).unwrap_or(Err(TypesetError::Panicked));
    let made = match result {
        Ok(made) => Some(made),
        Err(cause) => {
            let reason = cause.source().map(tracing::field::display);
            tracing::debug!(latex = %job.latex, %cause, reason, "a formula could not be typeset");
            None
        }
    };
    Done { job, made }
}

fn make(ctx: &egui::Context, job: &Job) -> Result<Made, TypesetError> {
    let raster = typeset(&job.latex, job.display, job.em_px, job.max_side)?;
    let [width, height] = raster.size;
    let pixels = egui::ColorImage::from_rgba_unmultiplied(raster.size, &raster.rgba);
    let texture = ctx.load_texture(
        format!("math:{}", job.latex),
        pixels,
        egui::TextureOptions::LINEAR,
    );
    let pixels_per_point = job.pixels_per_point;
    let points = |em: f64| (em * f64::from(job.em_px) / f64::from(pixels_per_point)) as f32;
    let image = MathImage {
        texture: texture.id(),
        texture_size: egui::vec2(width as f32, height as f32) / pixels_per_point,
        bleed: raster.bleed_px as f32 / pixels_per_point,
        width: points(raster.width_em),
        ascent: points(raster.ascent_em),
        descent: points(raster.descent_em),
    };
    Ok(Made {
        _texture: texture,
        image,
        bytes: width * height * BYTES_PER_PIXEL,
    })
}
