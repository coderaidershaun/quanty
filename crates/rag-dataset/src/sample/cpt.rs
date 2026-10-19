//! The records of the continued pre-training files: one line of text with where it came from,
//! how good it is and how it was made.

use rag_core::{ConceptId, DocId, ItemId};
use serde::{Deserialize, Serialize};

use crate::documents::{DocRole, Split};
use crate::section::{ImageRef, Provenance, Section};

use super::PIPELINE_VERSION;

/// What a text is: the book as printed, or one of the ways a model rewrote it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CptKind {
    Raw,
    RephraseWiki,
    RephraseLecture,
    RephraseQa,
    Entigraph,
    NotationVariant,
    Exercise,
}

impl CptKind {
    /// The spelling in the files and in sample ids.
    pub fn as_str(self) -> &'static str {
        match self {
            CptKind::Raw => "raw",
            CptKind::RephraseWiki => "rephrase_wiki",
            CptKind::RephraseLecture => "rephrase_lecture",
            CptKind::RephraseQa => "rephrase_qa",
            CptKind::Entigraph => "entigraph",
            CptKind::NotationVariant => "notation_variant",
            CptKind::Exercise => "exercise",
        }
    }
}

/// `<section id>:<kind>`, with `:<n>` after it when a section has several samples of one kind.
pub fn sample_id(section_id: &str, kind: CptKind, n: Option<u32>) -> String {
    match n {
        Some(n) => format!("{section_id}:{}:{n}", kind.as_str()),
        None => format!("{section_id}:{}", kind.as_str()),
    }
}

/// Where a text came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub media: String,
    pub document_id: DocId,
    pub document: String,
    pub doc_role: DocRole,
    pub section_path: Vec<String>,
    pub pages: Vec<u32>,
    pub item_ids: Vec<ItemId>,
    pub concept_ids: Vec<ConceptId>,
}

impl Source {
    pub fn of(section: &Section, concept_ids: &[ConceptId]) -> Source {
        Source {
            media: section.document.media.clone(),
            document_id: section.document.id,
            document: section.document.title.clone(),
            doc_role: section.document.role,
            section_path: section.section_path.clone(),
            pages: section.pages.clone(),
            item_ids: section.item_ids.clone(),
            concept_ids: concept_ids.to_vec(),
        }
    }
}

/// What the checks found out about a text. A check that was not run, or that does not apply,
/// leaves its field null.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Quality {
    /// The share of the words of the worst page that the page's text layer also holds.
    pub page_score: Option<f64>,
    pub latex_renders: Option<bool>,
    pub code_parses: Option<bool>,
    pub judge_score: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dedup {
    pub sha256: Option<String>,
    pub minhash_cluster: Option<String>,
}

/// How a text that a model wrote was made.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gen {
    pub model: Option<String>,
    /// The role and its version, such as `rephrase_wiki@1`.
    pub prompt_id: Option<String>,
    pub faithfulness_checked: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CptSample {
    pub id: String,
    pub kind: CptKind,
    pub source: Source,
    pub text: String,
    pub images: Vec<ImageRef>,
    pub n_tokens: usize,
    /// Always true: the count is the rough one of four characters to a token.
    pub tokens_estimated: bool,
    pub provenance: Provenance,
    pub quality: Quality,
    pub dedup: Dedup,
    // `gen` is a reserved word in Rust, so the field is written `r#gen`. The files still call it
    // `gen`.
    pub r#gen: Gen,
    pub split: Split,
    pub pipeline_version: String,
}

impl CptSample {
    /// The section as the book prints it, with what the checks found out about it.
    pub fn raw(
        section: &Section,
        quality: Quality,
        dedup: Dedup,
        concept_ids: &[ConceptId],
    ) -> CptSample {
        CptSample {
            id: sample_id(&section.id, CptKind::Raw, None),
            kind: CptKind::Raw,
            source: Source::of(section, concept_ids),
            text: section.markdown.clone(),
            images: section.images.clone(),
            n_tokens: section.n_tokens,
            tokens_estimated: true,
            provenance: section.provenance.clone(),
            quality,
            dedup,
            r#gen: Gen::default(),
            split: section.document.split,
            pipeline_version: PIPELINE_VERSION.to_owned(),
        }
    }
}

/// A text that the checks kept out of the dataset, with the reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RejectedSample {
    #[serde(flatten)]
    pub sample: CptSample,
    /// The words of the reason, such as `page_score` or `duplicate_of <id>`.
    pub rejected: String,
}
