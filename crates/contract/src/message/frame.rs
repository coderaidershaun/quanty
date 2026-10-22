//! Socket messages carry events and completion separately.
use super::{Event, RequestId};

/// Messages returned over the socket, including completion without an event.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[allow(
    clippy::large_enum_variant,
    reason = "The wire carries event values directly"
)]
#[serde(tag = "frame", content = "payload", rename_all = "snake_case")]
pub enum Frame {
    Event(Event),
    Finished(RequestId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Connection {
    Connected,
    Reconnecting { attempt: u32 },
    Lost,
}
