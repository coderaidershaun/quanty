//! Fills throwaway stores by hand with items and concepts at distances from one question that the
//! test chooses, so that the score of every item is known before the search runs.

use std::path::{Path, PathBuf};

use graph::{
    ConceptNode, DocumentNode, FalkorGraph, GraphStore, ItemNode, Mention, Relation, RelationKind,
};
use rag_core::{ConceptId, ConceptPoint, DocId, ItemId, ItemKind, ItemPayload, ItemPoint};
use rag_ingestion::Stores;
use rag_ingestion::testing::{StandInEmbedder, first_axis, vector_at};

/// The question that every item and concept of a [`Fixture`] is placed against.
pub const QUESTION: &str = "How is the price of an option found?";

/// An item to put in the stores, and how near to the question it is.
pub struct Placed {
    document: DocId,
    kind: ItemKind,
    text: String,
    cosine: f32,
    label: Option<String>,
    cites: Vec<String>,
    picture: Option<PathBuf>,
}

impl Placed {
    fn new(document: DocId, kind: ItemKind, text: &str, cosine: f32) -> Placed {
        Placed {
            document,
            kind,
            text: text.to_owned(),
            cosine,
            label: None,
            cites: Vec::new(),
            picture: None,
        }
    }

    pub fn chunk(document: DocId, text: &str, cosine: f32) -> Placed {
        Placed::new(document, ItemKind::Chunk, text, cosine)
    }

    pub fn formula(document: DocId, text: &str, cosine: f32, label: &str) -> Placed {
        Placed {
            label: Some(label.to_owned()),
            ..Placed::new(document, ItemKind::Formula, text, cosine)
        }
    }

    pub fn figure(document: DocId, text: &str, cosine: f32, label: &str, picture: &Path) -> Placed {
        Placed {
            label: Some(label.to_owned()),
            picture: Some(picture.to_owned()),
            ..Placed::new(document, ItemKind::Figure, text, cosine)
        }
    }

    /// The labels of the figures, tables and equations that the text points at.
    pub fn citing(self, labels: &[&str]) -> Placed {
        Placed {
            cites: labels.iter().map(|label| (*label).to_owned()).collect(),
            ..self
        }
    }
}

struct FixtureDocument {
    id: DocId,
    title: String,
    items: Vec<ItemNode>,
}

/// Items, concepts and the graph between them, written to the stores by [`Fixture::store_in`].
/// Each item and concept has its own axis of the vector, so its cosine to [`QUESTION`] is exactly
/// the cosine it was given.
pub struct Fixture {
    next_axis: usize,
    documents: Vec<FixtureDocument>,
    points: Vec<ItemPoint>,
    concept_nodes: Vec<ConceptNode>,
    concept_points: Vec<ConceptPoint>,
    mentions: Vec<Mention>,
    relations: Vec<Relation>,
}

impl Fixture {
    pub fn new() -> Fixture {
        Fixture {
            next_axis: 1,
            documents: Vec::new(),
            points: Vec::new(),
            concept_nodes: Vec::new(),
            concept_points: Vec::new(),
            mentions: Vec::new(),
            relations: Vec::new(),
        }
    }

    /// An embedder that puts [`QUESTION`] where every cosine of this fixture is measured from.
    pub fn embedder() -> StandInEmbedder {
        StandInEmbedder::default().placing(QUESTION, first_axis())
    }

    pub fn document(&mut self, title: &str) -> DocId {
        let id = DocId::from_source_sha256(&format!("fixture-{title}"));
        self.documents.push(FixtureDocument {
            id,
            title: title.to_owned(),
            items: Vec::new(),
        });
        id
    }

    pub fn add(&mut self, placed: Placed) -> ItemId {
        let axis = self.take_axis();
        let document = self
            .documents
            .iter_mut()
            .find(|document| document.id == placed.document)
            .expect("the document should have been made by this fixture");
        let position = u32::try_from(document.items.len()).expect("a few items");
        let id = ItemId::new(placed.document, placed.kind, position);
        document.items.push(ItemNode {
            id,
            kind: placed.kind,
            page: 1,
            printed_page: Some("1".to_owned()),
        });
        self.points.push(ItemPoint {
            id,
            vector: vector_at(placed.cosine, axis),
            payload: ItemPayload {
                doc_id: placed.document,
                doc_title: document.title.clone(),
                page: 1,
                printed_page: Some("1".to_owned()),
                kind: placed.kind,
                text: placed.text,
                image_path: placed.picture,
                label: placed.label,
                cites: placed.cites,
            },
        });
        id
    }

    /// A concept that the graph and the concept collection both hold.
    pub fn concept(&mut self, name: &str, cosine: f32) -> ConceptId {
        let id = self.concept_point(name, cosine);
        self.concept_nodes.push(ConceptNode {
            id,
            name: name.to_owned(),
            normalised_name: name.to_lowercase(),
            definition: format!("{name} in one line"),
        });
        id
    }

    /// A concept that only the concept collection holds: a search can find it by its distance to
    /// the question, and no item mentions it.
    pub fn concept_point(&mut self, name: &str, cosine: f32) -> ConceptId {
        let id = ConceptId::random();
        let axis = self.take_axis();
        self.concept_points.push(ConceptPoint {
            id,
            vector: vector_at(cosine, axis),
            name: name.to_owned(),
            aliases: Vec::new(),
        });
        id
    }

    pub fn mention(&mut self, item: ItemId, concept: ConceptId) {
        self.mentions.push(Mention {
            item,
            concept,
            wording: "as the item wrote it".to_owned(),
        });
    }

    pub fn relate(
        &mut self,
        stated_by: ItemId,
        from: ConceptId,
        kind: RelationKind,
        to: ConceptId,
    ) {
        self.relations.push(Relation {
            from,
            to,
            kind,
            item: stated_by,
        });
    }

    /// Creates both collections and writes everything that was added.
    pub async fn store_in(&self, stores: &Stores<FalkorGraph>) {
        stores
            .items
            .ensure_collection()
            .await
            .expect("the item collection should be created");
        stores
            .concepts
            .ensure_collection()
            .await
            .expect("the concept collection should be created");
        stores
            .items
            .upsert(&self.points)
            .await
            .expect("the items should be stored");
        for point in &self.concept_points {
            stores
                .concepts
                .upsert(point)
                .await
                .expect("the concept should be stored");
        }
        for document in &self.documents {
            let node = DocumentNode {
                id: document.id,
                title: document.title.clone(),
            };
            let graph = &stores.graph;
            graph.upsert_document(&node).await.unwrap();
            graph
                .upsert_items(document.id, &document.items)
                .await
                .unwrap();
        }
        for concept in &self.concept_nodes {
            stores.graph.upsert_concept(concept).await.unwrap();
        }
        stores.graph.add_mentions(&self.mentions).await.unwrap();
        stores.graph.add_relations(&self.relations).await.unwrap();
    }

    fn take_axis(&mut self) -> usize {
        let axis = self.next_axis;
        self.next_axis += 1;
        axis
    }
}
