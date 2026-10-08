//! The decision log of a run, and what it says about the mentions in the graph.

use std::fs;

use graph::testing::{StoredConceptGraph, StoredMention};
use rag_core::Config;
use serde_json::Value;

/// A log that was never written has no lines.
pub fn decisions_in(config: &Config) -> Vec<Value> {
    let text = match fs::read_to_string(&config.concept_decision_log) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("the decision log should be readable: {error}"),
    };
    text.lines()
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|error| {
                panic!("a line of the decision log is not JSON: {line}: {error}")
            })
        })
        .collect()
}

pub fn mentions_of<'a>(stored: &'a StoredConceptGraph, item: &str) -> Vec<&'a StoredMention> {
    stored
        .mentions
        .iter()
        .filter(|mention| mention.item == item)
        .collect()
}

/// The line of the log that decided a mention: the name that `item` used was linked to `concept`
/// (`matched`) or made it (`created`). `concept` is the id of the concept node.
pub fn decision_for_mention<'a>(log: &'a [Value], item: &str, concept: &str) -> Option<&'a Value> {
    log.iter().find(|line| {
        line["item"] == item && (line["matched"]["id"] == concept || line["created"] == concept)
    })
}
