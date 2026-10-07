//! The picture cache: one entry for each path, the order pictures are decoded in, and the byte
//! budget that keeps the textures from growing without bound.

use std::collections::HashMap;

use eframe::egui;

use super::decode::{DecodeJob, Decoded, Loaded, decode};
use super::picture::{ImageFailure, ImageState, Picture};
use crate::contract::ImageRef;
use crate::media::{Offload, Urgency, Workers};

const DEFAULT_BUDGET: usize = 256 * 1024 * 1024;
const DECODE_THREADS: usize = 2;
/// A stored size is never longer than this, whatever the graphics card allows: it is plenty for
/// a page.
const MAX_STORED_SIDE: u32 = 4096;

/// Pictures decoded off the window's thread, and the textures they became.
pub struct Images {
    workers: Workers<DecodeJob, Decoded>,
    entries: HashMap<ImageRef, Entry>,
    budget: usize,
    /// The bytes of every stored size of every ready picture.
    bytes: usize,
    /// The number of polls so far. A picture is wanted when it was asked for since the last poll.
    polls: u64,
    max_side: u32,
}

struct Entry {
    slot: Slot,
    /// The number of polls so far when a panel last asked for the picture.
    last_used: u64,
    urgency: Urgency,
}

enum Slot {
    Loading,
    Ready {
        picture: Picture,
        /// Dropping these frees the textures.
        textures: Vec<egui::TextureHandle>,
    },
    Failed(ImageFailure),
}

impl Entry {
    fn is_loading(&self) -> bool {
        matches!(self.slot, Slot::Loading)
    }

    fn bytes(&self) -> usize {
        match &self.slot {
            Slot::Ready { textures, .. } => {
                textures.iter().map(egui::TextureHandle::byte_size).sum()
            }
            Slot::Loading | Slot::Failed(_) => 0,
        }
    }
}

impl Images {
    /// A cache that keeps at most 256 MiB of textures.
    pub fn new(ctx: &egui::Context, offload: Offload) -> Images {
        Images {
            workers: Workers::start(
                offload,
                "quanty-images",
                DECODE_THREADS,
                ctx.clone(),
                decode,
            ),
            entries: HashMap::new(),
            budget: DEFAULT_BUDGET,
            bytes: 0,
            polls: 0,
            max_side: max_side(ctx),
        }
    }

    /// Keeps at most `bytes` of textures, except for the pictures that are on screen.
    pub fn with_budget(mut self, bytes: usize) -> Images {
        self.budget = bytes;
        self
    }

    /// Once per frame, before any panel draws: takes finished pictures, drops queued work that
    /// nobody asked for since the last poll, and frees the least recently used pictures.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let last_drawn = self.polls;
        self.polls += 1;
        self.max_side = max_side(ctx);
        while let Some(decoded) = self.workers.take_done() {
            self.store(decoded);
        }
        self.cancel_unwanted(last_drawn);
        self.evict(last_drawn);
    }

    /// With `Offload::Manual`: decodes every queued picture on the calling thread.
    pub fn run_pending(&mut self) {
        self.workers.run_pending();
    }

    /// True when no picture is loading.
    pub fn is_idle(&self) -> bool {
        !self.entries.values().any(Entry::is_loading)
    }

    /// The picture, wanted on screen now. The first call queues the decode and answers
    /// `Loading`. It never blocks and never reads a file.
    pub fn get(&mut self, image: &ImageRef) -> ImageState {
        let Some(entry) = self.entries.get_mut(image) else {
            self.request(image, Urgency::Now);
            return ImageState::Loading;
        };
        entry.last_used = self.polls;
        match &entry.slot {
            Slot::Ready { picture, .. } => ImageState::Ready(*picture),
            Slot::Failed(failure) => ImageState::Failed(*failure),
            Slot::Loading => {
                if entry.urgency == Urgency::Later {
                    entry.urgency = Urgency::Now;
                    self.move_to_front(image);
                }
                ImageState::Loading
            }
        }
    }

    /// The same as `get`, but decoded after everything that is on screen. Call it each frame for
    /// the pictures that will be asked for next.
    pub fn prefetch(&mut self, image: &ImageRef) {
        match self.entries.get_mut(image) {
            Some(entry) => entry.last_used = self.polls,
            None => self.request(image, Urgency::Later),
        }
    }

    /// Forgets a failure, so the next `get` reads the file again.
    pub fn retry(&mut self, image: &ImageRef) {
        let failed = self
            .entries
            .get(image)
            .is_some_and(|entry| matches!(entry.slot, Slot::Failed(_)));
        if failed {
            self.entries.remove(image);
        }
    }

    fn request(&mut self, image: &ImageRef, urgency: Urgency) {
        let entry = Entry {
            slot: Slot::Loading,
            last_used: self.polls,
            urgency,
        };
        self.entries.insert(image.clone(), entry);
        self.workers.submit(self.job_for(image), urgency);
    }

    /// The queue cannot move a job, so a queued one is taken out and put in again at the front.
    /// A job that already runs is not in the queue and is left alone.
    fn move_to_front(&self, image: &ImageRef) {
        if self.workers.cancel(|job| job.path == image.path) > 0 {
            self.workers.submit(self.job_for(image), Urgency::Now);
        }
    }

    fn job_for(&self, image: &ImageRef) -> DecodeJob {
        DecodeJob {
            path: image.path.clone(),
            max_side: self.max_side,
        }
    }

    fn store(&mut self, decoded: Decoded) {
        let image = ImageRef { path: decoded.path };
        let Some(entry) = self.entries.get_mut(&image) else {
            return;
        };
        debug_assert!(entry.is_loading(), "one job for each entry");
        entry.slot = match decoded.outcome {
            Ok(Loaded { size, textures }) => {
                let levels = textures
                    .iter()
                    .map(|texture| (texture.id(), texture.size()[0] as u32));
                let picture = Picture::new(size, levels);
                Slot::Ready { picture, textures }
            }
            Err(failure) => Slot::Failed(failure),
        };
        self.bytes += entry.bytes();
    }

    /// A queued job that nobody asked for since the last poll is removed, and so is its entry.
    /// A job that already runs is left alone: its entry stays until the result arrives.
    fn cancel_unwanted(&mut self, last_drawn: u64) {
        let unwanted: Vec<ImageRef> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_loading() && entry.last_used < last_drawn)
            .map(|(image, _)| image.clone())
            .collect();
        if unwanted.is_empty() {
            return;
        }
        let mut cancelled = Vec::new();
        self.workers.cancel(|job| {
            let hit = unwanted.iter().any(|image| image.path == job.path);
            if hit {
                cancelled.push(ImageRef {
                    path: job.path.clone(),
                });
            }
            hit
        });
        for image in cancelled {
            self.entries.remove(&image);
        }
    }

    /// While over budget, frees the ready picture that was used longest ago. A picture that
    /// was asked for since the last poll is on screen and stays, even when it alone is over.
    fn evict(&mut self, last_drawn: u64) {
        if self.bytes <= self.budget {
            return;
        }
        let mut idle: Vec<(u64, ImageRef)> = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                matches!(entry.slot, Slot::Ready { .. }) && entry.last_used < last_drawn
            })
            .map(|(image, entry)| (entry.last_used, image.clone()))
            .collect();
        idle.sort_by_key(|(last_used, _)| *last_used);
        for (_, image) in idle {
            if self.bytes <= self.budget {
                break;
            }
            if let Some(entry) = self.entries.remove(&image) {
                self.bytes -= entry.bytes();
            }
        }
    }
}

/// The longest side a stored size may have.
fn max_side(ctx: &egui::Context) -> u32 {
    let allowed = ctx.input(|input| input.max_texture_side);
    u32::try_from(allowed).map_or(MAX_STORED_SIDE, |side| side.min(MAX_STORED_SIDE))
}
