//! What the plan says for each section: how many texts of which kind, which relations to explain
//! and how many questions, budgeted by how rarely the library mentions the section's concepts,
//! through `build` and `plan_for` as another crate calls them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use graph::{ConceptNode, DocumentNode, FalkorGraph, ItemNode, Mention, Relation, RelationKind};
use ocr::read_chapter;
use rag_core::{ConceptId, DocumentLabels};
use rag_dataset::documents::ExportDocument;
use rag_dataset::plan::{ConceptSeed, GenerationPlan, RelationSeed, SectionPlan, plan_for};
use rag_dataset::sample::CptKind;
use rag_dataset::section::{Section, sections_of};
use rag_dataset::{BuildOptions, build};
use rag_ingestion::testing::{StandInLlm, ThrowawayStores};
use rag_ingestion::{Item, chapter_items};

use crate::support;

fn read_plan(dataset: &Path) -> GenerationPlan {
    let saved = fs::read_to_string(dataset.join("plan.json")).expect("plan.json should be written");
    serde_json::from_str(&saved).expect("plan.json should hold a plan")
}

/// The texts of a section: this many of each of the three rephrased kinds, and these others.
fn texts(rephrased_each: u32, others: &[(CptKind, u32)]) -> BTreeMap<CptKind, u32> {
    let rephrased = [
        CptKind::RephraseWiki,
        CptKind::RephraseLecture,
        CptKind::RephraseQa,
    ];
    let each = rephrased.map(|kind| (kind, rephrased_each));
    each.into_iter().chain(others.iter().copied()).collect()
}

/// The texts a section plans without the notation variant, which about one section in twenty gets.
fn texts_without_notation(section: &SectionPlan) -> BTreeMap<CptKind, u32> {
    let mut kept = section.rewrites.clone();
    kept.remove(&CptKind::NotationVariant);
    kept
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_graph_every_section_gets_the_counts_of_the_options() {
    let work = tempfile::tempdir().unwrap();
    let out = work.path().join("t");
    let mut options = support::options_over(&support::sample_content(), &out);
    options.generation = false;
    let llm = StandInLlm::finding_nothing();

    build(&options, &llm, None, &mut |_, _| {}).await.unwrap();

    let plan = read_plan(&out);
    assert!(!plan.graph_read, "no graph was given");
    assert_eq!(
        plan.sections.len(),
        3,
        "one section for each sample chapter"
    );
    // The three rewrites of the options, one of each kind, and the exercise that a section of
    // common rarity gets.
    let counts_of_the_options = texts(1, &[(CptKind::Exercise, 1)]);
    for section in &plan.sections {
        let id = &section.section_id;
        assert!(section.concept_ids.is_empty(), "{id} has concepts");
        assert!(section.relations.is_empty(), "{id} has relations");
        assert_eq!(section.questions, options.questions, "{id}");
        assert_eq!(
            texts_without_notation(section),
            counts_of_the_options,
            "{id}"
        );
    }
    let raw = support::read_jsonl(&out.join("cpt/raw.jsonl"));
    assert_eq!(raw.len(), 3);
    for record in &raw {
        assert_eq!(record["source"]["concept_ids"], serde_json::json!([]));
    }
}

/// A sample chapter as a build exports it: its document, the items that ingestion makes of it and
/// its one section.
struct SampleChapter {
    document: ExportDocument,
    items: Vec<Item>,
    section: Section,
}

impl SampleChapter {
    fn at(chapter_folder: &Path) -> SampleChapter {
        let document = support::exported_at(&support::sample_content(), chapter_folder);
        let chapter = read_chapter(chapter_folder).unwrap();
        let items = chapter_items(&chapter);
        let mut sections = sections_of(&document, &chapter, &items);
        // The numbers of these tests count on one section that holds every item.
        assert_eq!(sections.len(), 1, "{} is cut in sections", document.title);
        let section = sections.remove(0);
        let item_ids: Vec<_> = items.iter().map(|item| item.id).collect();
        assert_eq!(
            section.item_ids, item_ids,
            "{} misses items",
            document.title
        );
        SampleChapter {
            document,
            items,
            section,
        }
    }

    async fn write_to(&self, graph: &FalkorGraph) {
        let document = DocumentNode {
            id: self.document.id,
            title: self.document.title.clone(),
            labels: DocumentLabels::default(),
        };
        let items: Vec<ItemNode> = self
            .items
            .iter()
            .map(|item| ItemNode {
                id: item.id,
                kind: item.payload.kind,
                page: item.payload.page,
                printed_page: item.payload.printed_page.clone(),
            })
            .collect();
        graph.upsert_document(&document).await.unwrap();
        graph.upsert_items(document.id, &items).await.unwrap();
    }

    fn planned_in<'a>(&self, plan: &'a GenerationPlan) -> &'a SectionPlan {
        let id = &self.section.id;
        let found = plan
            .sections
            .iter()
            .find(|section| &section.section_id == id);
        found.unwrap_or_else(|| panic!("the plan has no section {id}"))
    }
}

fn concept(name: &str) -> ConceptNode {
    ConceptNode {
        id: ConceptId::random(),
        name: name.to_owned(),
        normalised_name: name.to_lowercase(),
        definition: format!("{name} in one line"),
    }
}

fn mention(item: &Item, concept: &ConceptNode) -> Mention {
    Mention {
        item: item.id,
        concept: concept.id,
        wording: concept.name.to_lowercase(),
    }
}

fn relation(from: &ConceptNode, kind: RelationKind, to: &ConceptNode, item: &Item) -> Relation {
    Relation {
        from: from.id,
        to: to.id,
        kind,
        item: item.id,
    }
}

/// The three sample chapters in a throwaway graph. The book chapter is trained on, and the first
/// chapter of the notes is held out.
///
/// - Five common concepts: each is mentioned by the first six items of the book chapter, and the
///   first of them also by the first item of the held-out notes.
/// - Four rare concepts: each is mentioned by one item of the second chapter of the notes.
/// - Five relations among the rare concepts, one of each kind, that items of that chapter state,
///   and a sixth that the held-out notes state.
struct Library {
    stores: ThrowawayStores,
    graph: FalkorGraph,
    book: SampleChapter,
    held_out_notes: SampleChapter,
    notes: SampleChapter,
    common: [ConceptNode; 5],
    rare: [ConceptNode; 4],
    /// The seeds that the five relations of the training notes should give.
    trained_relations: Vec<RelationSeed>,
    held_out_relation: Relation,
}

impl Library {
    async fn new(test_name: &str) -> Library {
        let stores = ThrowawayStores::new(test_name);
        let graph = FalkorGraph::connect(stores.config())
            .await
            .expect("FalkorDB should answer");
        let book = SampleChapter::at(&support::sample_chapter());
        let held_out_notes = SampleChapter::at(&support::intuition_chapter());
        let notes = SampleChapter::at(&support::in_depth_chapter());
        for chapter in [&book, &held_out_notes, &notes] {
            chapter.write_to(&graph).await;
        }

        let common = [
            "Option premium",
            "Strike price",
            "Expiration date",
            "Underlying asset",
            "Intrinsic value",
        ]
        .map(concept);
        let rare = [
            "Itô's lemma",
            "Risk-neutral measure",
            "Girsanov theorem",
            "Martingale",
        ]
        .map(concept);
        for written in common.iter().chain(&rare) {
            graph.upsert_concept(written).await.unwrap();
        }
        let mut mentions = Vec::new();
        for concept in &common {
            mentions.extend(book.items[..6].iter().map(|item| mention(item, concept)));
        }
        mentions.push(mention(&held_out_notes.items[0], &common[0]));
        mentions.extend(
            notes
                .items
                .iter()
                .zip(&rare)
                .map(|(item, c)| mention(item, c)),
        );
        graph.add_mentions(&mentions).await.unwrap();

        let [r1, r2, r3, r4] = &rare;
        let trained = [
            relation(r1, RelationKind::DerivedFrom, r2, &notes.items[0]),
            relation(r1, RelationKind::Assumes, r3, &notes.items[1]),
            relation(r2, RelationKind::Generalises, r3, &notes.items[2]),
            relation(r3, RelationKind::PartOf, r4, &notes.items[3]),
            relation(r4, RelationKind::UsedFor, r1, &notes.items[4]),
        ];
        let held_out_relation = relation(r2, RelationKind::Assumes, r4, &held_out_notes.items[0]);
        graph.add_relations(&trained).await.unwrap();
        graph
            .add_relations(std::slice::from_ref(&held_out_relation))
            .await
            .unwrap();
        let seed = |id: ConceptId| {
            let node = rare
                .iter()
                .find(|node| node.id == id)
                .expect("a rare concept");
            ConceptSeed {
                id,
                name: node.name.clone(),
                definition: node.definition.clone(),
            }
        };
        let trained_relations = trained
            .iter()
            .map(|relation| RelationSeed {
                kind: relation.kind.as_str().to_owned(),
                from: seed(relation.from),
                to: seed(relation.to),
                item: relation.item,
            })
            .collect();

        Library {
            stores,
            graph,
            book,
            held_out_notes,
            notes,
            common,
            rare,
            trained_relations,
            held_out_relation,
        }
    }

    fn options(&self) -> BuildOptions {
        let out = self.stores.temporary_folder().join("t");
        let mut options = support::options_over(&support::sample_content(), &out);
        options.generation = false;
        options.selection.holdout = vec![self.held_out_notes.document.id.to_string()];
        options
    }

    /// A dry run over the sample chapters that reads this graph, and the plan it wrote.
    async fn plan_of_a_build(&self, options: &BuildOptions) -> GenerationPlan {
        let llm = StandInLlm::finding_nothing();
        let graph = Some(&self.graph);
        build(options, &llm, graph, &mut |_, _| {}).await.unwrap();
        read_plan(&options.out)
    }
}

/// The records of both files carry the concepts that the plan found for their section.
fn assert_records_carry_the_concepts_of_the_plan(plan: &GenerationPlan, out: &Path) {
    let planned: BTreeMap<String, serde_json::Value> = plan
        .sections
        .iter()
        .map(|section| {
            let concepts = serde_json::to_value(&section.concept_ids).unwrap();
            (format!("{}:raw", section.section_id), concepts)
        })
        .collect();
    for (file, count) in [("cpt/raw.jsonl", 2), ("cpt/heldout.jsonl", 1)] {
        let records = support::read_jsonl(&out.join(file));
        assert_eq!(records.len(), count, "{file}");
        for record in records {
            let id = record["id"].as_str().unwrap();
            assert_eq!(record["source"]["concept_ids"], planned[id], "{id}");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-dataset --test integration -- --ignored plan::"]
async fn rare_concepts_get_more_texts_and_questions_and_only_training_items_seed_relations() {
    let library = Library::new("plan-rarity").await;

    let plan = library.plan_of_a_build(&library.options()).await;

    assert!(plan.graph_read);
    let book = library.book.planned_in(&plan);
    let notes = library.notes.planned_in(&plan);
    let held_out = library.held_out_notes.planned_in(&plan);

    // The notes name concepts that one item mentions, and the book names concepts that six or
    // seven mention, so the notes get three times the options and the book exactly the options.
    let relation_texts = (CptKind::Entigraph, 5);
    let rare_texts = texts(3, &[relation_texts, (CptKind::Exercise, 1)]);
    assert_eq!(
        texts_without_notation(notes),
        rare_texts,
        "rare concepts get more texts"
    );
    assert_eq!(notes.questions, 6, "rare concepts get more questions");
    let common_texts = texts(1, &[(CptKind::Exercise, 1)]);
    assert_eq!(
        texts_without_notation(book),
        common_texts,
        "common concepts get the options"
    );
    assert_eq!(book.questions, 2);

    let as_set = |ids: &[ConceptId]| ids.iter().copied().collect::<BTreeSet<_>>();
    let ids_of = |nodes: &[ConceptNode]| nodes.iter().map(|node| node.id).collect();
    assert_eq!(as_set(&book.concept_ids), ids_of(&library.common));
    assert_eq!(as_set(&notes.concept_ids), ids_of(&library.rare));
    assert_eq!(held_out.concept_ids, [library.common[0].id]);

    // The five relations of the training notes, one of each kind.
    let by_kind = |mut seeds: Vec<RelationSeed>| {
        seeds.sort_by_key(|seed| seed.kind.clone());
        seeds
    };
    assert_eq!(
        by_kind(notes.relations.clone()),
        by_kind(library.trained_relations.clone())
    );
    // The sixth was stated by a held-out item, so no text of the training data may explain it.
    let [_, r2, _, r4] = &library.rare;
    for seed in plan.sections.iter().flat_map(|section| &section.relations) {
        assert_ne!(seed.item, library.held_out_relation.item, "{seed:?}");
        assert!(!(seed.from.id == r2.id && seed.to.id == r4.id), "{seed:?}");
    }
    assert!(book.relations.is_empty() && held_out.relations.is_empty());
    assert!(
        held_out.rewrites.is_empty(),
        "a held-out section is not trained on"
    );
    assert_eq!(held_out.questions, 2);

    assert_records_carry_the_concepts_of_the_plan(&plan, &library.options().out);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-dataset --test integration -- --ignored plan::"]
async fn the_caps_keep_the_rarest_sections_whole() {
    let library = Library::new("plan-caps").await;
    let mut options = library.options();
    options.max_sft = 7;
    options.max_synthetic = Some(10);

    let plan = library.plan_of_a_build(&options).await;

    let book = library.book.planned_in(&plan);
    let notes = library.notes.planned_in(&plan);
    let held_out = library.held_out_notes.planned_in(&plan);
    let position = |section: &SectionPlan| plan.sections.iter().position(|other| other == section);
    // A cap that is filled in the order of the sections would favour the book.
    assert!(
        position(book) < position(notes),
        "the book is not before the notes"
    );

    // The notes are the rarest section: they keep the six questions they asked for, and the book
    // gets the one that is left. The held-out section is outside the cap.
    assert_eq!(notes.questions, 6);
    assert_eq!(book.questions, 1);
    assert_eq!(held_out.questions, 2);
    assert_eq!(plan.totals.questions, 9);

    // The notes fill the ten texts: the rephrased texts first, then one text about a relation.
    // The exercise and the notation variant are what is cut first.
    assert_eq!(notes.rewrites, texts(3, &[(CptKind::Entigraph, 1)]));
    assert!(book.rewrites.is_empty(), "nothing is left for the book");
    assert_eq!(plan.totals.texts.values().sum::<u32>(), 10);
    for section in &plan.sections {
        let explained = *section.rewrites.get(&CptKind::Entigraph).unwrap_or(&0);
        let id = &section.section_id;
        assert_eq!(explained as usize, section.relations.len(), "{id}");
    }
}

/// How many copies of the section the notation rule is tried on.
const COPIES: usize = 400;

/// The section of the hand-written second chapter that holds a display formula.
fn section_with_a_display_formula() -> Section {
    let document = support::exported_at(&support::sample_content(), &support::in_depth_chapter());
    let chapter = read_chapter(&document.folder).unwrap();
    sections_of(&document, &chapter, &chapter_items(&chapter))
        .into_iter()
        .find(|section| section.markdown.lines().any(|line| line == "$$"))
        .expect("the second chapter of the notes holds a display formula")
}

/// Copies of the section under ids that differ, each with this text.
fn copies_of(section: &Section, markdown: &str) -> Vec<Section> {
    (0..COPIES)
        .map(|number| Section {
            id: format!("{}:{number}", section.id),
            markdown: markdown.to_owned(),
            ..section.clone()
        })
        .collect()
}

fn ids_with_a_notation_variant(plan: &GenerationPlan) -> BTreeSet<&str> {
    plan.sections
        .iter()
        .filter(|section| section.rewrites.contains_key(&CptKind::NotationVariant))
        .map(|section| section.section_id.as_str())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn about_one_section_with_a_display_formula_in_twenty_gets_a_notation_variant() {
    let formula = section_with_a_display_formula();
    let with_formulas = copies_of(&formula, &formula.markdown);
    let without_formulas: Vec<&str> = formula.markdown.lines().filter(|l| *l != "$$").collect();
    let plain = copies_of(&formula, &without_formulas.join("\n"));
    let options = support::options_over(&support::sample_content(), Path::new("unused"));

    let first = plan_for(&with_formulas, None, &options).await.unwrap();
    let second = plan_for(&with_formulas, None, &options).await.unwrap();
    let picked = ids_with_a_notation_variant(&first);
    // A rerun that picks other sections would not find the texts it asked for before.
    assert_eq!(picked, ids_with_a_notation_variant(&second));
    let one_in_twenty = COPIES / 20;
    assert!(
        (5..=40).contains(&picked.len()),
        "{} picked, not about {one_in_twenty}",
        picked.len()
    );
    for section in &first.sections {
        let variants = section.rewrites.get(&CptKind::NotationVariant);
        assert!(variants.is_none_or(|count| *count == 1), "{variants:?}");
    }

    // The same ids, and no display formula to rewrite.
    let plain_plan = plan_for(&plain, None, &options).await.unwrap();
    assert_eq!(ids_with_a_notation_variant(&plain_plan), BTreeSet::new());
}
