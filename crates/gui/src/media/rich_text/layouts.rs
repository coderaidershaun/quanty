//! The text that was read and laid out, kept between frames: a block is read once for its
//! content and laid out once for each width and scale, and dropped when it is not drawn.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;

use super::RichText;
use super::flow::{Flow, break_lines};
use super::measure::{WaitFor, alone_width, measure};
use super::paint;
use super::parse::{Parsed, parse};
use super::parse_table::{ParsedTable, parse_table};
use super::style::text_look;
use super::table::{self, Source, TableLayout};
use crate::media::math::Math;
use crate::theme::TextRole;

/// A layout that was not drawn for this many polls is dropped.
const UNDRAWN_POLLS: u64 = 2;
/// Texts that were read are kept up to this many. The least recently used go first.
const MAX_PARSED: usize = 1024;

/// What a layout is for: the content, the width in whole points, and the display scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LayoutKey {
    content: u64,
    wrap: u32,
    scale: u32,
}

impl LayoutKey {
    /// A width with no limit, or one that is not a number, still makes a key: the first is
    /// kept as the largest whole number and the second as zero.
    fn new(content: u64, wrap: f32, pixels_per_point: f32) -> LayoutKey {
        LayoutKey {
            content,
            wrap: wrap.floor().max(0.0) as u32,
            scale: pixels_per_point.to_bits(),
        }
    }

    /// The width the layout is made for.
    fn width(self) -> f32 {
        self.wrap as f32
    }
}

/// Counts what the text layouts did, for tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutStats {
    pub texts_parsed: u64,
    pub layouts_built: u64,
    pub layouts_held: usize,
}

#[derive(Debug)]
struct Used<T> {
    value: T,
    /// The number of the poll in which it was last used.
    last_used: u64,
}

/// The laid-out text that is kept between frames.
#[derive(Debug, Default)]
pub struct Layouts {
    parsed: HashMap<u64, Used<Arc<Parsed>>>,
    /// `None` is a text that was found not to be a table.
    parsed_tables: HashMap<u64, Used<Option<Arc<ParsedTable>>>>,
    flows: HashMap<LayoutKey, Used<Arc<Flow>>>,
    tables: HashMap<LayoutKey, Used<Arc<TableLayout>>>,
    /// The number of polls so far.
    polls: u64,
    stats: LayoutStats,
}

impl Layouts {
    /// An empty cache.
    pub fn new() -> Layouts {
        Layouts::default()
    }

    /// Once per frame, before any panel draws: drops what was not drawn lately.
    pub fn poll(&mut self, _ctx: &egui::Context) {
        self.polls += 1;
        let polls = self.polls;
        self.flows
            .retain(|_, used| polls - used.last_used < UNDRAWN_POLLS);
        self.tables
            .retain(|_, used| polls - used.last_used < UNDRAWN_POLLS);
        keep_most_recent(&mut self.parsed);
        keep_most_recent(&mut self.parsed_tables);
    }

    /// What the cache has done so far, and what it holds now.
    pub fn stats(&self) -> LayoutStats {
        LayoutStats {
            layouts_held: self.flows.len() + self.tables.len(),
            ..self.stats
        }
    }

    /// The layout of a block of text at the width that is left in `ui`. It is made again when a
    /// formula that is on screen is not what the layout was made with.
    pub(super) fn block(
        &mut self,
        ui: &egui::Ui,
        math: &mut Math,
        text: &RichText<'_>,
    ) -> Arc<Flow> {
        let content = egui::util::hash((
            std::mem::discriminant(&text.role),
            text.markdown,
            text.cites,
        ));
        let key = LayoutKey::new(content, ui.available_width(), ui.ctx().pixels_per_point());
        let view = paint::view_of(ui);
        if let Some(used) = self.flows.get_mut(&key) {
            used.last_used = self.polls;
            if !paint::is_stale(&used.value, math, view) {
                return Arc::clone(&used.value);
            }
        }
        let parsed = self.read(content, text.markdown);
        let look = text_look(text.role);
        let measured = measure(ui, &parsed, text.cites, &look, math, WaitFor::OwnFormulas);
        let piece_width = alone_width(ui);
        let flow = Arc::new(break_lines(&measured, key.width(), &look, &piece_width));
        self.stats.layouts_built += 1;
        let used = Used {
            value: Arc::clone(&flow),
            last_used: self.polls,
        };
        self.flows.insert(key, used);
        flow
    }

    /// The layout of a table at the width that is left in `ui`, or `None` when the text is not
    /// a table.
    pub(super) fn table(
        &mut self,
        ui: &egui::Ui,
        math: &mut Math,
        markdown: &str,
        role: TextRole,
    ) -> Option<Arc<TableLayout>> {
        let content = egui::util::hash((std::mem::discriminant(&role), markdown));
        let parsed = self.read_table(content, markdown)?;
        let key = LayoutKey::new(content, ui.available_width(), ui.ctx().pixels_per_point());
        let view = paint::view_of(ui);
        if let Some(used) = self.tables.get_mut(&key) {
            used.last_used = self.polls;
            if !table::is_stale(&used.value, math, view) {
                return Some(Arc::clone(&used.value));
            }
        }
        let source = Source {
            table: &parsed,
            role,
            content,
            width: key.width(),
        };
        let layout = Arc::new(table::build(ui, math, &source));
        self.stats.layouts_built += 1;
        let used = Used {
            value: Arc::clone(&layout),
            last_used: self.polls,
        };
        self.tables.insert(key, used);
        Some(layout)
    }

    fn read_table(&mut self, content: u64, markdown: &str) -> Option<Arc<ParsedTable>> {
        let polls = self.polls;
        let used = self.parsed_tables.entry(content).or_insert_with(|| {
            self.stats.texts_parsed += 1;
            Used {
                value: parse_table(markdown).map(Arc::new),
                last_used: polls,
            }
        });
        used.last_used = polls;
        used.value.clone()
    }

    fn read(&mut self, content: u64, markdown: &str) -> Arc<Parsed> {
        let polls = self.polls;
        let used = self.parsed.entry(content).or_insert_with(|| {
            self.stats.texts_parsed += 1;
            Used {
                value: Arc::new(parse(markdown)),
                last_used: polls,
            }
        });
        used.last_used = polls;
        Arc::clone(&used.value)
    }
}

/// Drops the entries that were used least recently, until `MAX_PARSED` are left.
fn keep_most_recent<T>(parsed: &mut HashMap<u64, Used<T>>) {
    if parsed.len() <= MAX_PARSED {
        return;
    }
    let mut by_use: Vec<(u64, u64)> = parsed
        .iter()
        .map(|(content, used)| (used.last_used, *content))
        .collect();
    by_use.sort_unstable();
    let surplus = parsed.len() - MAX_PARSED;
    for (_, content) in by_use.into_iter().take(surplus) {
        parsed.remove(&content);
    }
}
