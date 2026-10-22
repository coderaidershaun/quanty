//! The picture cache: one entry for each URL, the order pictures are decoded in, and the byte
//! budget that keeps the textures from growing without bound.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use eframe::egui;
use web_time::Instant;

use super::decode::{DecodeJob, Decoded, Loaded, decode};
use super::picture::{ImageFailure, ImageState, Picture};
use crate::media::{Fetch, Offload, Urgency, Workers};
use contract::ImageRef;

const BUDGET: usize = 256 * 1024 * 1024;
/// A stored size is never longer than this, whatever the graphics card allows: it is plenty for
/// a page.
const MAX_STORED_SIDE: u32 = 4096;

pub struct Images {
    workers: Workers<DecodeJob, Decoded>,
    fetch: Box<dyn Fetch>,
    fetched: Receiver<Fetched>,
    sender: Sender<Fetched>,
    ctx: egui::Context,
    entries: HashMap<ImageRef, Entry>,
    /// The bytes of every stored size of every ready picture.
    bytes: usize,
    /// A picture is wanted when it was asked for since the last poll.
    polls: u64,
    max_side: u32,
}

struct Fetched {
    image: ImageRef,
    result: Result<Vec<u8>, String>,
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
    pub fn new(ctx: &egui::Context, offload: Offload, fetch: Box<dyn Fetch>) -> Images {
        let (sender, fetched) = channel();
        Images {
            workers: Workers::start(offload, ctx.clone(), decode),
            fetch,
            fetched,
            sender,
            ctx: ctx.clone(),
            entries: HashMap::new(),
            bytes: 0,
            polls: 0,
            max_side: max_side(ctx),
        }
    }

    /// Call this once per frame, before any panel draws.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let last_drawn = self.polls;
        self.polls += 1;
        self.max_side = max_side(ctx);
        self.take_fetched();
        while let Some(decoded) = self.workers.take_done() {
            self.store(decoded);
        }
        self.cancel_unwanted(last_drawn);
        self.evict(last_drawn);
    }

    /// With `Offload::Manual`: decodes every queued picture on the calling thread.
    pub fn run_pending(&mut self) {
        self.take_fetched();
        self.workers.run_pending();
    }

    pub fn run_for(&mut self, duration: Duration) {
        let started = Instant::now();
        self.take_fetched();
        self.workers
            .run_for(duration.saturating_sub(started.elapsed()));
    }

    pub fn is_idle(&self) -> bool {
        !self.entries.values().any(Entry::is_loading)
    }

    /// It never blocks and never reads a file.
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

    /// Call it each frame for the pictures that will be asked for next.
    pub fn prefetch(&mut self, image: &ImageRef) {
        match self.entries.get_mut(image) {
            Some(entry) => entry.last_used = self.polls,
            None => self.request(image, Urgency::Later),
        }
    }

    /// Forgets a failure, so the next `get` fetches the picture again.
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
        let sender = self.sender.clone();
        let requested = image.clone();
        let ctx = self.ctx.clone();
        self.fetch.get(
            &image.url,
            Box::new(move |result| {
                let _ = sender.send(Fetched {
                    image: requested,
                    result,
                });
                ctx.request_repaint();
            }),
        );
    }

    fn move_to_front(&mut self, image: &ImageRef) {
        self.workers.promote(|job| job.url == image.url);
    }

    fn take_fetched(&mut self) {
        while let Ok(fetched) = self.fetched.try_recv() {
            let Some(entry) = self.entries.get_mut(&fetched.image) else {
                continue;
            };
            self.workers.submit(
                DecodeJob {
                    url: fetched.image.url,
                    bytes: fetched.result,
                    max_side: self.max_side,
                },
                entry.urgency,
            );
        }
    }

    fn store(&mut self, decoded: Decoded) {
        let image = ImageRef { url: decoded.url };
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
            let hit = unwanted.iter().any(|image| image.url == job.url);
            if hit {
                cancelled.push(ImageRef {
                    url: job.url.clone(),
                });
            }
            hit
        });
        for image in cancelled {
            self.entries.remove(&image);
        }
    }

    /// A picture that was asked for since the last poll is on screen and stays, even when it
    /// alone is over the budget.
    fn evict(&mut self, last_drawn: u64) {
        if self.bytes <= BUDGET {
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
            if self.bytes <= BUDGET {
                break;
            }
            if let Some(entry) = self.entries.remove(&image) {
                self.bytes -= entry.bytes();
            }
        }
    }
}

fn max_side(ctx: &egui::Context) -> u32 {
    let allowed = ctx.input(|input| input.max_texture_side);
    u32::try_from(allowed).map_or(MAX_STORED_SIDE, |side| side.min(MAX_STORED_SIDE))
}
