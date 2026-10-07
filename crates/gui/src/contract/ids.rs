//! The ids that name a request, a notice, and the documents, items and concepts the stores hold.

use uuid::Uuid;

/// Issued by the shared state, counting from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RequestId(pub u64);

/// Issued by the shared state, counting from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NoticeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DocId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ItemId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ConceptId(pub Uuid);
