//! The records that a dataset writes, line by line, and the ids that name them. The field names
//! are the names in the files, so a change to one is a change to every reader of a dataset.

mod cpt;
mod sft;

pub use cpt::{CptKind, CptSample, Dedup, Gen, Quality, RejectedSample, Source, sample_id};
pub use sft::{
    Message, Reasoning, Seed, SeedKind, SftSample, Speaker, Task, Verification, VerificationMethod,
    sft_id,
};

/// The `pipeline_version` of every record and of the manifest: the version of this crate.
pub const PIPELINE_VERSION: &str = env!("CARGO_PKG_VERSION");
