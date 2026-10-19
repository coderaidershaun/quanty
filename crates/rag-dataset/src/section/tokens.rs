//! The rough token count that every size in the crate is measured in.

/// Four characters to a token, the rule that ingestion uses to size its chunks. It is an
/// estimate, so every record that carries a count also says that it is estimated.
pub fn estimated_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}
