//! Decides which stored concept an extracted concept belongs to: one with the same name or alias,
//! else the nearest one by meaning, judged by its score or, in between, by what the model says.

use graph::{ConceptAlias, ConceptNode, GraphStore};
use rag_core::{
    ConceptHit, ConceptId, ConceptPoint, EmbedError, Embedder, Embedding, ItemId, Llm, LlmError,
    UsageTally, concept_input, embedding_usage,
};

use super::ask::Outcome;
use super::cache::Cache;
use super::decision_log::{DecisionLog, Naming, Rule};
use super::question::ExtractedConcept;
use super::{ConceptError, ConceptExtractor, ConceptSummary};
use crate::stores::Stores;

/// A stored concept at or over this score is the same concept. It is this high because at 0.90,
/// counterparts such as a call and a put were linked with no question, and in one run with real
/// vectors such pairs scored up to 0.93.
pub const LINK_SCORE: f32 = 0.95;
/// A stored concept under this score is a different concept. Between the two, the model is asked.
pub const ASK_SCORE: f32 = 0.75;

pub(super) enum Resolved {
    Created(ConceptId),
    Linked(ConceptId),
    /// The model cannot answer any question until the person acts. Nothing was written for this
    /// concept.
    Stopped(LlmError),
}

/// The form in which names are compared: lower case, with every run of characters that are not
/// letters or digits turned into one space. A hyphen is a word break, not a letter to delete, so
/// "Black–Scholes", "black-scholes" and "Black Scholes" are the same name.
pub(super) fn normalised(name: &str) -> String {
    let mut normalised = String::new();
    let mut after_break = false;
    for character in name.to_lowercase().chars() {
        if !character.is_alphanumeric() {
            after_break = true;
            continue;
        }
        if after_break && !normalised.is_empty() {
            normalised.push(' ');
        }
        after_break = false;
        normalised.push(character);
    }
    normalised
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Band {
    High,
    Middle,
    Low,
}

impl Band {
    fn of(score: f32) -> Band {
        if score >= LINK_SCORE {
            Band::High
        } else if score >= ASK_SCORE {
            Band::Middle
        } else {
            Band::Low
        }
    }
}

enum Judgement {
    Same(Rule, ConceptNode),
    Different(Rule),
    Stopped(LlmError),
}

pub(super) struct Resolver<'a, L, E, G> {
    pub extractor: &'a ConceptExtractor<L>,
    pub cache: &'a Cache,
    pub embedder: &'a E,
    pub stores: &'a Stores<G>,
    pub log: &'a DecisionLog,
}

impl<L: Llm, E: Embedder, G: GraphStore> Resolver<'_, L, E, G> {
    // SMELL: finding a concept and creating it are two calls, so two ingests that run at the same
    // time can each create the same concept.
    /// Links the concept that `item` named to the stored concept that is the same, or makes a new
    /// one. A linked name becomes an alias of the stored concept. Each decision is added to the
    /// decision log after its writes succeeded, the questions that were asked are counted in the
    /// summary, and what the embedding and the questions used is added to `spent`.
    ///
    /// # Errors
    /// - [`ConceptError::Embed`] when the concept cannot be embedded
    /// - [`ConceptError::ConceptStore`] and [`ConceptError::Graph`] when a store cannot be read or
    ///   written
    /// - [`ConceptError::PointWithoutNode`] when the concepts collection holds a concept that the
    ///   graph does not
    /// - [`ConceptError::Comparison`] when the model failed twice to compare two concepts
    /// - [`ConceptError::Cache`] and [`ConceptError::DecisionLog`] when the cache folder or the
    ///   decision log cannot be used
    pub(super) async fn resolve(
        &self,
        item: ItemId,
        concept: &ExtractedConcept,
        summary: &mut ConceptSummary,
        spent: &mut UsageTally,
    ) -> Result<Resolved, ConceptError> {
        let normalised_name = normalised(&concept.name);
        let naming = Naming {
            item,
            name: &concept.name,
        };
        if let Some(stored) = self
            .stores
            .graph
            .find_concept_by_name(&normalised_name)
            .await?
        {
            let decision = naming.linked(Rule::ExactName, &stored, None);
            self.log.append(&decision)?;
            return Ok(Resolved::Linked(stored.id));
        }

        let vector = self.embed(concept, spent).await?;
        // SMELL: only the nearest stored concept is compared, so a name whose true match is the
        // second nearest becomes a new concept. Measured on real concepts, that happens for one or
        // two of eight true pairs, depending on the order of the names.
        let nearest = self
            .stores
            .concepts
            .nearest(vector.clone(), 1)
            .await?
            .into_iter()
            .next();
        let Some(hit) = nearest else {
            let id = self.create(concept, normalised_name, vector).await?;
            let decision = naming.created(Rule::NothingStored, id, None);
            self.log.append(&decision)?;
            return Ok(Resolved::Created(id));
        };

        match self.judge(concept, &hit, summary, spent).await? {
            Judgement::Stopped(error) => Ok(Resolved::Stopped(error)),
            Judgement::Same(rule, stored) => {
                self.link(concept, normalised_name, &hit).await?;
                let decision = naming.linked(rule, &stored, Some(hit.score));
                self.log.append(&decision)?;
                Ok(Resolved::Linked(hit.id))
            }
            Judgement::Different(rule) => {
                let id = self.create(concept, normalised_name, vector).await?;
                let decision = naming.created(rule, id, Some(&hit));
                self.log.append(&decision)?;
                Ok(Resolved::Created(id))
            }
        }
    }

    /// The vector of the concept, from the same form of text that stored concepts are embedded in.
    async fn embed(
        &self,
        concept: &ExtractedConcept,
        spent: &mut UsageTally,
    ) -> Result<Embedding, ConceptError> {
        let embed_error = |source| ConceptError::Embed {
            name: concept.name.clone(),
            source,
        };
        let inputs = [concept_input(&concept.name, &concept.definition)];
        let vectors = self
            .embedder
            .embed_document(&inputs)
            .await
            .map_err(embed_error)?;
        spent.add_all(&embedding_usage(&inputs));
        let [vector] = <[Embedding; 1]>::try_from(vectors).map_err(|vectors| {
            embed_error(EmbedError::WrongVectorCount {
                expected: 1,
                got: vectors.len(),
            })
        })?;
        Ok(vector)
    }

    async fn judge(
        &self,
        concept: &ExtractedConcept,
        hit: &ConceptHit,
        summary: &mut ConceptSummary,
        spent: &mut UsageTally,
    ) -> Result<Judgement, ConceptError> {
        let band = Band::of(hit.score);
        if band == Band::Low {
            return Ok(Judgement::Different(Rule::LowScore));
        }
        // The definition is in the graph only, and the node also shows that the collection and
        // the graph belong together.
        let Some(stored) = self.stores.graph.concept(hit.id).await? else {
            return Err(ConceptError::PointWithoutNode {
                collection: self.stores.concepts.collection().to_owned(),
                name: hit.name.clone(),
                id: hit.id,
            });
        };
        if band == Band::High {
            return Ok(Judgement::Same(Rule::HighScore, stored));
        }

        let reading = self
            .extractor
            .same_concept(self.cache, concept, &stored)
            .await?;
        summary.llm_calls += reading.llm_calls;
        spent.add_all(&reading.usage);
        let same = match reading.outcome {
            Outcome::Cached(same) => {
                summary.cache_hits += 1;
                same
            }
            Outcome::Asked(same) => same,
            // SMELL: a comparison that fails every time stops every ingest of this document, and
            // nothing skips it. A guess would be worse: "different" makes a second concept that
            // nothing can merge, and "same" makes a wrong alias that every later item matches.
            Outcome::Failed(reason) => {
                return Err(ConceptError::Comparison {
                    new_name: concept.name.clone(),
                    stored_name: stored.name,
                    reason,
                });
            }
            Outcome::Stopped(error) => return Ok(Judgement::Stopped(error)),
        };
        Ok(if same {
            Judgement::Same(Rule::LlmSame, stored)
        } else {
            Judgement::Different(Rule::LlmDifferent)
        })
    }

    /// Adds the name to the concept in the collection and then in the graph. The collection comes
    /// first: a run that stops between the two finds no exact match next time, comes back here
    /// with the same score and the kept answer, and writes both again.
    async fn link(
        &self,
        concept: &ExtractedConcept,
        normalised_name: String,
        hit: &ConceptHit,
    ) -> Result<(), ConceptError> {
        let mut aliases = hit.aliases.clone();
        if !aliases.contains(&concept.name) {
            aliases.push(concept.name.clone());
        }
        // SMELL: the aliases of the point are read by the search and written back whole, so of two
        // ingests that run at the same time, one can drop the alias that the other added to the
        // point. The graph keeps both.
        self.stores.concepts.set_aliases(hit.id, &aliases).await?;
        let alias = ConceptAlias {
            concept: hit.id,
            name: concept.name.clone(),
            normalised_name,
        };
        self.stores.graph.add_alias(&alias).await?;
        Ok(())
    }

    /// Writes a new concept to the graph and then to the concepts collection. The graph comes
    /// first, because a point with no node stops every later ingest that names a concept near it.
    async fn create(
        &self,
        concept: &ExtractedConcept,
        normalised_name: String,
        vector: Embedding,
    ) -> Result<ConceptId, ConceptError> {
        let id = ConceptId::random();
        let node = ConceptNode {
            id,
            name: concept.name.clone(),
            normalised_name,
            definition: concept.definition.clone(),
        };
        self.stores.graph.upsert_concept(&node).await?;
        // SMELL: a run that stops between the two writes leaves a concept with no point, and so does
        // a concept that was made before the concepts collection existed. Such a concept is matched
        // by its exact name only, and nothing repairs it.
        let point = ConceptPoint {
            id,
            vector,
            name: concept.name.clone(),
            aliases: Vec::new(),
        };
        self.stores.concepts.upsert(&point).await?;
        Ok(id)
    }
}
