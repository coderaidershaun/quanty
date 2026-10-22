//! Sample metadata is validated locally so the fake stays independent of backend crates.

use crate::contract::{ArtifactMetadata, ChapterItem};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    page: u32,
    number: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    id: String,
    kind: String,
    pieces: Vec<Position>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    signature: String,
    manifest: Vec<Target>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    title: String,
    description: String,
    model: String,
    cache_identity: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    format_version: u32,
    source: Source,
    generation_signature: String,
    entries: Vec<Entry>,
}

pub(super) fn attach(folder: &Path, items: &mut [ChapterItem]) {
    if let Some(saved) = validated(folder, items) {
        for item in items {
            let entry = saved.entries.iter().find(|entry| {
                saved.source.manifest.iter().any(|target| {
                    target.id == entry.id
                        && target.pieces.first().is_some_and(|position| {
                            position.page == item.page && position.number == item.piece.number
                        })
                })
            });
            item.metadata = entry.map(|entry| ArtifactMetadata {
                title: entry.title.clone(),
                description: entry.description.clone(),
            });
        }
    }
}

fn validated(folder: &Path, items: &[ChapterItem]) -> Option<Saved> {
    let saved: Saved =
        serde_json::from_str(&std::fs::read_to_string(folder.join("artifacts.json")).ok()?).ok()?;
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(folder.join("chapter.json")).ok()?).ok()?;
    let sha = index["source-sha256"].as_str()?;
    let mut pieces = Vec::new();
    for page in 1..=index["page-count"].as_u64()? {
        let page_folder = folder.join(format!("page-num-{page}"));
        let file: Value =
            serde_json::from_str(&std::fs::read_to_string(page_folder.join("page.json")).ok()?)
                .ok()?;
        for entry in file["pieces"].as_array()? {
            let content =
                std::fs::read_to_string(page_folder.join(entry["file"].as_str()?)).ok()?;
            let mut detail = entry.clone();
            let object = detail.as_object_mut()?;
            object.remove("file");
            object.remove("number");
            normalize(&mut detail);
            pieces.push(json!({"page":page,"number":entry["number"],"printed":file["printed-page-number"],"detail":detail,"content":content.strip_suffix('\n').unwrap_or(&content)}));
        }
    }
    let manifest: Vec<_> = items
        .iter()
        .map(|item| Target {
            id: format!("{sha}:{}:{}", item.page, item.piece.number),
            kind: match item.piece.kind {
                crate::contract::PieceKind::Formula => "formula",
                crate::contract::PieceKind::Figure => "figure",
                crate::contract::PieceKind::Table => "table",
                _ => "code",
            }
            .to_owned(),
            pieces: vec![Position {
                page: item.page,
                number: item.piece.number,
            }],
        })
        .collect();
    let mut hash = Sha256::new();
    let mut content = json!({"media":index["media-title"],"name":index["name"],"pieces":pieces});
    content.sort_all_objects();
    hash.update(content.to_string().as_bytes());
    hash.update(serde_json::to_vec(&manifest).ok()?);
    let signature: String = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if saved.format_version != 1
        || saved.source.signature != signature
        || saved.source.manifest != manifest
    {
        return None;
    }
    let known: BTreeSet<_> = manifest.iter().map(|target| target.id.as_str()).collect();
    if known.len() != manifest.len() {
        return None;
    }
    let mut seen = BTreeSet::new();
    for entry in &saved.entries {
        if !known.contains(entry.id.as_str())
            || !seen.insert(entry.id.as_str())
            || !plain(&entry.title, 80)
            || !plain(&entry.description, 280)
        {
            return None;
        }
        let _provenance = (&entry.model, &entry.cache_identity);
    }
    let _policy = &saved.generation_signature;
    Some(saved)
}

fn plain(text: &str, limit: usize) -> bool {
    !text.trim().is_empty()
        && text == text.trim()
        && text.chars().count() <= limit
        && !text
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
        && !has_formatting(text)
}

fn has_formatting(text: &str) -> bool {
    let prefix = if text.starts_with('#') {
        text.trim_start_matches('#')
    } else if text.starts_with(['*', '-', '+']) {
        &text[1..]
    } else {
        ""
    };
    if text.contains('`') || prefix.starts_with(char::is_whitespace) {
        return true;
    }
    for (start, _) in text.match_indices('*') {
        let before = text[..start].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '*')) {
            continue;
        }
        let width = text[start..].bytes().take_while(|&b| b == b'*').count();
        let rest = &text[start + width..];
        if rest.starts_with(char::is_whitespace) || rest.is_empty() {
            continue;
        }
        let delimiter = &text[start..start + width];
        for (end, _) in rest.match_indices(delimiter) {
            let content = &rest[..end];
            let after = rest[end + width..].chars().next();
            if !content.is_empty()
                && !content.ends_with(|c: char| c.is_whitespace() || c == '*')
                && after.is_none_or(|c| !c.is_alphanumeric() && !matches!(c, '_' | '*'))
            {
                return true;
            }
        }
    }
    false
}

fn normalize(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("image");
        object.retain(|_, value| {
            !value.is_null()
                && value != ""
                && value.as_array().is_none_or(|array| !array.is_empty())
        });
        for child in object.values_mut() {
            normalize(child);
        }
    }
}
