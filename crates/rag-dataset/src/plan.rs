//! Decides what the models are asked to write for each section: how many rewritten texts of which
//! kind, which relations between concepts to explain, and how many questions.

use std::collections::BTreeMap;

use graph::FalkorGraph;
use rag_core::{ConceptId, ItemId};
use serde::{Deserialize, Serialize};

use crate::documents::Split;
use crate::options::BuildOptions;
use crate::sample::CptKind;
use crate::section::Section;

/// The most questions a held-out section gets: enough to measure a model, too few to train on.
const HELDOUT_QUESTIONS: u32 = 2;
/// The kinds that the rewrites of one section take turns at.
const REWRITE_KINDS: [CptKind; 3] = [
    CptKind::RephraseWiki,
    CptKind::RephraseLecture,
    CptKind::RephraseQa,
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationPlan {
    /// One for each kept section, in the order of the sections.
    pub sections: Vec<SectionPlan>,
    pub totals: PlanTotals,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionPlan {
    pub section_id: String,
    pub concept_ids: Vec<ConceptId>,
    /// How many texts of each kind to write. A kind with none is left out.
    pub rewrites: BTreeMap<CptKind, u32>,
    pub relations: Vec<RelationSeed>,
    pub questions: u32,
}

/// A relation between two concepts that a text can explain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationSeed {
    /// The name of the edge, such as `ASSUMES`.
    pub kind: String,
    pub from: ConceptSeed,
    pub to: ConceptSeed,
    /// The item that stated the relation.
    pub item: ItemId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConceptSeed {
    pub id: ConceptId,
    pub name: String,
    pub definition: String,
}

/// What the plan comes to, for a dry run to print.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTotals {
    pub texts: BTreeMap<CptKind, u32>,
    pub questions: u32,
    /// Each text and its judge, each question with its solver samples and its judge.
    pub model_calls: u32,
}

#[derive(thiserror::Error, Debug)]
pub enum PlanError {
    #[error(
        "could not read the graph to plan the generation; check that FalkorDB is running, or build without the graph"
    )]
    Graph(#[from] graph::GraphError),
}

/// Not built yet: the same counts for every training section, taken from the options, and no
/// concepts or relations, because the graph is not read.
///
/// # Errors
/// [`PlanError::Graph`] when the graph cannot be read.
pub async fn plan_for(
    sections: &[Section],
    _graph: Option<&FalkorGraph>,
    options: &BuildOptions,
) -> Result<GenerationPlan, PlanError> {
    tracing::info!(
        "the plan is not built yet, so every section gets the counts of the options and the graph is not read"
    );
    let section_plans: Vec<SectionPlan> = sections
        .iter()
        .map(|section| {
            let (rewrites, questions) = match section.document.split {
                Split::Train => (rewrites_for(options.rewrites), options.questions),
                Split::Heldout => (BTreeMap::new(), options.questions.min(HELDOUT_QUESTIONS)),
            };
            SectionPlan {
                section_id: section.id.clone(),
                concept_ids: Vec::new(),
                rewrites,
                relations: Vec::new(),
                questions,
            }
        })
        .collect();
    let totals = totals_of(&section_plans, options.samples);
    Ok(GenerationPlan {
        sections: section_plans,
        totals,
    })
}

/// The rewrites take turns at the kinds, so three give one of each.
fn rewrites_for(count: u32) -> BTreeMap<CptKind, u32> {
    let kinds = REWRITE_KINDS.len() as u32;
    REWRITE_KINDS
        .iter()
        .zip(0..)
        .map(|(kind, turn)| (*kind, count / kinds + u32::from(turn < count % kinds)))
        .filter(|(_, texts)| *texts > 0)
        .collect()
}

fn totals_of(sections: &[SectionPlan], samples: u32) -> PlanTotals {
    let mut totals = PlanTotals::default();
    for section in sections {
        for (kind, count) in &section.rewrites {
            let texts = totals.texts.entry(*kind).or_insert(0);
            *texts = texts.saturating_add(*count);
        }
        totals.questions = totals.questions.saturating_add(section.questions);
    }
    let texts = totals
        .texts
        .values()
        .fold(0u32, |sum, n| sum.saturating_add(*n));
    totals.model_calls = texts
        .saturating_mul(2)
        .saturating_add(samples.saturating_add(2).saturating_mul(totals.questions));
    totals
}
