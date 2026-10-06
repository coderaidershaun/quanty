//! The identifier of a concept. A concept has no source to compute an id from, so its id is drawn
//! at random once and the concept is found again by its name.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConceptId(Uuid);

impl ConceptId {
    /// A new identifier that no other concept has.
    pub fn random() -> ConceptId {
        ConceptId(Uuid::new_v4())
    }
}

impl fmt::Display for ConceptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ConceptId {
    type Err = uuid::Error;

    fn from_str(text: &str) -> Result<ConceptId, uuid::Error> {
        text.parse().map(ConceptId)
    }
}
