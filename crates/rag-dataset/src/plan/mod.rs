//! Decides what the models are asked to write for each section: how many rewritten texts of which
//! kind, which relations between concepts to explain, and how many questions.

mod budget;
mod rarity;

use std::collections::BTreeMap;

use graph::FalkorGraph;
use rag_core::{ConceptId, ItemId};
use serde::{Deserialize, Serialize};

use crate::options::BuildOptions;
use crate::sample::CptKind;
use crate::section::Section;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationPlan {
    /// False when the build did not read the graph. Then no section has concepts or relations,
    /// and every section has the budget of a section of common rarity.
    pub graph_read: bool,
    /// One for each kept section, in the order of the sections.
    pub sections: Vec<SectionPlan>,
    pub totals: PlanTotals,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionPlan {
    pub section_id: String,
    /// The concepts that the items of the section mention.
    pub concept_ids: Vec<ConceptId>,
    /// How many texts of each kind to write. A kind with none is left out.
    pub rewrites: BTreeMap<CptKind, u32>,
    /// The relations to explain, the rarest first. Only a training section has any, and an item of
    /// a training section stated each.
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

/// Plans every kept section: the concepts its items mention, how many texts of which kind and how
/// many questions it gets, and which relations between its concepts a text should explain. A
/// section whose concepts few items mention gets more than one whose concepts many items mention.
/// Without the graph, no concept or relation is known and every section is planned as one of
/// common rarity.
///
/// # Errors
/// [`PlanError::Graph`] when the graph cannot be read.
pub async fn plan_for(
    sections: &[Section],
    graph: Option<&FalkorGraph>,
    options: &BuildOptions,
) -> Result<GenerationPlan, PlanError> {
    let rarities = match graph {
        Some(graph) => rarity::read(sections, graph).await?,
        None => {
            tracing::info!(
                "the graph is not read, so no section has concepts or relations to explain, and every section is planned as one of common rarity"
            );
            rarity::without_graph(sections)
        }
    };
    let section_plans = budget::plan_sections(sections, rarities, options);
    let totals = budget::totals_of(&section_plans, options.samples);
    Ok(GenerationPlan {
        graph_read: graph.is_some(),
        sections: section_plans,
        totals,
    })
}
