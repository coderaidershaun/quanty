//! Stand-ins and throwaway stores for the tests of this crate and of the crates that read what it
//! stores. They are behind the cargo feature `testing`, so the product does not carry them, and
//! they panic with a clear message when something fails.

mod stand_in_embedder;
mod stand_in_llm;
mod throwaway_stores;

pub use stand_in_embedder::{StandInEmbedder, first_axis, vector_at};
pub use stand_in_llm::{AskedQuestion, StandInLlm};
pub use throwaway_stores::ThrowawayStores;
