//! The tools that explore the concept graph: find a concept, read one, follow its relations and
//! its items, compare documents by their concepts, and find items like an item. Each tool reads
//! the graph, and `find_concepts` by meaning and `similar_items` read Qdrant as well.

mod args;
mod concept;
mod documents;
mod error;
mod find;
mod items;
mod neighbours;
mod view;

pub(crate) use concept::{ConceptArgs, ConceptView, concept};
pub(crate) use documents::{
    CompareDocumentsArgs, CompareResult, DocumentConceptsArgs, DocumentConceptsResult,
    compare_documents, document_concepts,
};
pub(crate) use error::ConceptsError;
pub(crate) use find::{FindConceptsArgs, FindConceptsResult, find_concepts};
pub(crate) use items::{
    ConceptItemsArgs, ConceptItemsResult, SimilarItemsArgs, SimilarItemsResult, concept_items,
    similar_items,
};
pub(crate) use neighbours::{
    ConceptNeighboursArgs, ConceptPathArgs, NeighboursResult, PathResult, concept_neighbours,
    concept_path,
};
