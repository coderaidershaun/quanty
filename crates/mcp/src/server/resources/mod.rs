//! The library as MCP resources: templates for the addresses that can be read, a list of the media
//! and documents, reads that give what the matching tool gives, and the notice that the list of
//! resources changed.

mod error;
mod list;
mod notify;
mod read;
mod uri;

use rmcp::model::{CacheScope, ListResourceTemplatesResult};

use super::TOOL_LIST_TTL_MS;
pub(super) use list::list;
pub(super) use notify::{accepted_filter, listen, tell_session};
pub(super) use read::read;

/// The resource templates. They are the same for every caller, so a peer may keep them as long as
/// it keeps the list of tools.
pub(super) fn templates(sends_cache_hints: bool) -> ListResourceTemplatesResult {
    let templates = ListResourceTemplatesResult::with_all_items(uri::templates());
    if sends_cache_hints {
        templates
            .with_ttl_ms(TOOL_LIST_TTL_MS)
            .with_cache_scope(CacheScope::Public)
    } else {
        templates
    }
}
