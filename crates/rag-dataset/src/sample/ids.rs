//! How each sample made from a section is named, so that the same book gives the same names on
//! every run.

// SMELL: a sample id names the kind of the sample, and the records of continued pre-training take
// their ids from here, so this file and those records import each other.
use super::CptKind;

/// `<section id>:<kind>`, with `:<n>` after it when a section has several samples of one kind.
pub fn sample_id(section_id: &str, kind: CptKind, n: Option<u32>) -> String {
    match n {
        Some(n) => format!("{section_id}:{}:{n}", kind.as_str()),
        None => format!("{section_id}:{}", kind.as_str()),
    }
}

/// `sft:<section id>:<n>`, with the number in four digits.
pub fn sft_id(section_id: &str, n: u32) -> String {
    format!("sft:{section_id}:{n:04}")
}
