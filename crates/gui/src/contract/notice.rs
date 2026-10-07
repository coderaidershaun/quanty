//! A message the app keeps for the person after something finished: a document deleted, an
//! ingest done, a failure to read about later.

use super::failure::Failure;
use super::ids::NoticeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoticeKind {
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Notice {
    pub id: NoticeId,
    pub kind: NoticeKind,
    pub title: String,
    pub detail: String,
    pub failure: Option<Failure>,
    pub read: bool,
}
