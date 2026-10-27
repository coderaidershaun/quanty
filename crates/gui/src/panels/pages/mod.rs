//! The tabs that fill the whole page under the top bar: the Library, the Ingest page and the
//! Datasets page.

pub mod datasets;
pub mod ingest;
pub mod library;
mod media_card;
mod progress;

/// Below this width of a form, two fields that stand side by side stand one under the other.
const STACK_BELOW: f32 = 480.0;
