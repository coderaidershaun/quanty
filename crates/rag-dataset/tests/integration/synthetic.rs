//! The synthetic texts of a dataset: each draft a model writes is judged against the passage it was
//! written from, only a faithful one is written to `cpt/synthetic.jsonl`, and an answer once paid
//! for is never paid for again.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use ocr::{read_chapter, sha256_hex};
use rag_core::{ConceptId, LlmError, UsageTally};
use rag_dataset::documents::{ExportDocument, Split};
use rag_dataset::generate::asker::Asker;
use rag_dataset::generate::role::Role;
use rag_dataset::generate::{
    ENTIGRAPH, EXERCISE, JUDGE, NOTATION, REPHRASE_LECTURE, REPHRASE_QA, REPHRASE_WIKI,
    write_synthetic,
};
use rag_dataset::plan::{ConceptSeed, GenerationPlan, RelationSeed, SectionPlan};
use rag_dataset::sample::{CptKind, Source};
use rag_dataset::section::{Section, estimated_tokens, sections_of};
use rag_dataset::{DatasetFolder, GenerationKind, Step, Written};
use rag_ingestion::chapter_items;
use rag_ingestion::testing::{AskedQuestion, StandInLlm};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::support;

/// The one section of the in-depth chapter, the way a build makes it.
fn in_depth_section() -> Section {
    let document = support::exported_at(&support::sample_content(), &support::in_depth_chapter());
    let chapter = read_chapter(&support::in_depth_chapter()).unwrap();
    let mut sections = sections_of(&document, &chapter, &chapter_items(&chapter));
    assert_eq!(sections.len(), 1, "the chapter is one section");
    sections.remove(0)
}

/// The same section, in a document that is held out of training.
fn held_out(section: &Section) -> Section {
    Section {
        id: format!("{}-heldout", section.id),
        document: ExportDocument {
            split: Split::Heldout,
            ..section.document.clone()
        },
        ..section.clone()
    }
}

fn concept_id(number: u8) -> ConceptId {
    format!("00000000-0000-0000-0000-{number:012}")
        .parse()
        .unwrap()
}

fn concept(number: u8, name: &str, definition: &str) -> ConceptSeed {
    ConceptSeed {
        id: concept_id(number),
        name: name.to_owned(),
        definition: definition.to_owned(),
    }
}

/// `Black–Scholes model ASSUMES geometric Brownian motion`, stated by the section's first item.
fn assumes(section: &Section) -> RelationSeed {
    RelationSeed {
        kind: "ASSUMES".to_owned(),
        from: concept(
            1,
            "Black–Scholes model",
            "The model that prices a European option from five inputs.",
        ),
        to: concept(
            2,
            "geometric Brownian motion",
            "A price whose log changes by a steady drift and random shocks.",
        ),
        item: section.item_ids[0],
    }
}

fn section_plan(
    section: &Section,
    rewrites: &[(CptKind, u32)],
    relations: Vec<RelationSeed>,
) -> SectionPlan {
    SectionPlan {
        section_id: section.id.clone(),
        concept_ids: vec![concept_id(1), concept_id(2)],
        rewrites: rewrites.iter().copied().collect(),
        relations,
        ..Default::default()
    }
}

/// Two rewrites, an exercise, and the explanation of one relation.
fn four_texts(section: &Section) -> SectionPlan {
    section_plan(
        section,
        &[
            (CptKind::RephraseWiki, 1),
            (CptKind::RephraseLecture, 1),
            (CptKind::Exercise, 1),
            (CptKind::Entigraph, 1),
        ],
        vec![assumes(section)],
    )
}

fn work_of(plans: Vec<SectionPlan>, sections: Vec<Section>) -> Work {
    Work {
        plan: GenerationPlan {
            sections: plans,
            ..Default::default()
        },
        sections,
    }
}

/// The role a question was asked of.
fn role_of(question: &AskedQuestion) -> &'static Role {
    [
        &REPHRASE_WIKI,
        &REPHRASE_LECTURE,
        &REPHRASE_QA,
        &ENTIGRAPH,
        &EXERCISE,
        &NOTATION,
        &JUDGE,
    ]
    .into_iter()
    .find(|role| role.system_prompt == question.system_prompt)
    .expect("every question should be asked of one of the roles")
}

/// A stand-in whose writers give every input a text of its own, and whose judge passes every
/// draft with a 4. `exception` may give another reply for a role and its input.
fn drafting(
    exception: impl Fn(&Role, &str) -> Option<Result<Value, LlmError>> + Send + Sync + 'static,
) -> StandInLlm {
    StandInLlm::replying_to_questions(move |question, _| {
        let role = role_of(question);
        if let Some(reply) = exception(role, &question.input) {
            return reply;
        }
        if role.id == JUDGE.id {
            return Ok(json!({ "unsupported_claims": [], "faithful": true, "score": 4 }));
        }
        let fingerprint = sha256_hex(question.input.as_bytes());
        Ok(json!({ "text": format!("{} text {}", role.id, &fingerprint[..16]) }))
    })
}

/// What one run of `write_synthetic` did.
struct Run {
    written: Written,
    /// Every step the run told, with what the model had cost by then.
    steps: Vec<(Step, UsageTally)>,
    /// How many questions the asker sent to the model.
    asked: usize,
    usage: UsageTally,
}

/// The plan and the sections a run writes from.
struct Work {
    plan: GenerationPlan,
    sections: Vec<Section>,
}

impl Work {
    /// Writes into `out`, asking through `llm` with the answers kept in `cache`.
    async fn run(&self, llm: &StandInLlm, cache: &Path, out: &Path) -> Run {
        let asker = Asker::new(llm, cache).unwrap();
        let mut steps = Vec::new();
        let written = write_synthetic(
            &self.plan,
            &self.sections,
            &asker,
            &DatasetFolder::new(out),
            &mut |step, usage| steps.push((step, usage.clone())),
        )
        .await
        .unwrap();
        Run {
            written,
            steps,
            asked: asker.calls(),
            usage: asker.usage(),
        }
    }

    /// Writes into a new folder, with its own cache.
    async fn run_in_new_folder(&self, llm: &StandInLlm) -> (TempDir, Run) {
        let folder = tempfile::tempdir().unwrap();
        let run = self
            .run(llm, &folder.path().join("cache"), folder.path())
            .await;
        (folder, run)
    }
}

fn synthetic_records(out: &Path) -> Vec<Value> {
    support::read_jsonl(&out.join("cpt/synthetic.jsonl"))
}

fn ids_of(records: &[Value]) -> Vec<&str> {
    records
        .iter()
        .map(|record| record["id"].as_str().unwrap())
        .collect()
}

/// A count of one for each name.
fn once_each(names: &str) -> BTreeMap<String, usize> {
    names
        .split_whitespace()
        .map(|name| (name.to_owned(), 1))
        .collect()
}

#[tokio::test]
async fn judged_texts_are_written_with_their_kind_role_score_and_source() {
    let section = in_depth_section();
    let held_out = held_out(&section);
    let work = work_of(
        vec![
            four_texts(&section),
            section_plan(&held_out, &[(CptKind::RephraseWiki, 1)], Vec::new()),
        ],
        vec![section.clone(), held_out],
    );
    let llm = drafting(|_, _| None);

    let (folder, run) = work.run_in_new_folder(&llm).await;

    let records = synthetic_records(folder.path());
    assert_eq!(
        records.len(),
        4,
        "one text for each count and one for the relation, and none for the held-out section"
    );
    let source =
        serde_json::to_value(Source::of(&section, &[concept_id(1), concept_id(2)])).unwrap();
    let roles = ["rephrase_wiki", "rephrase_lecture", "exercise", "entigraph"];
    for (record, role) in records.iter().zip(roles) {
        let text = record["text"].as_str().unwrap();
        let n_tokens = estimated_tokens(text);
        assert!(text.starts_with(role), "{text}");
        let expected = json!({
            "id": format!("{}:{role}:1", section.id),
            "kind": role,
            "source": source,
            "text": text,
            "images": [],
            "n_tokens": n_tokens,
            "tokens_estimated": true,
            "provenance": { "transcribed_tokens": 0, "model_written_tokens": n_tokens },
            "quality": { "page_score": null, "latex_renders": null, "code_parses": null, "judge_score": 4 },
            "dedup": { "sha256": sha256_hex(text.as_bytes()), "minhash_cluster": null },
            "gen": { "model": "stand-in", "prompt_id": format!("{role}@1"), "faithfulness_checked": true },
            "split": "train",
            "pipeline_version": env!("CARGO_PKG_VERSION"),
        });
        assert_eq!(record, &expected);
    }
    let kept = once_each("entigraph exercise rephrase_lecture rephrase_wiki");
    assert_eq!(run.written.by_kind, kept);
    assert!(run.written.rejected.is_empty());

    let questions = llm.questions();
    assert_eq!(questions.len(), 8, "four writers and four judges");
    assert_eq!(
        llm.most_calls_at_once(),
        4,
        "the four drafts are written at once"
    );
    let input_of = |role: &Role| {
        let asked = questions
            .iter()
            .find(|question| question.system_prompt == role.system_prompt);
        asked.unwrap().input.clone()
    };
    assert!(
        !section.notes.is_empty() && input_of(&REPHRASE_WIKI).contains(&section.notes),
        "the writer is given the notes of the section"
    );
    let relation = input_of(&ENTIGRAPH);
    for expected in [
        "Black–Scholes model ASSUMES geometric Brownian motion",
        "The model that prices a European option from five inputs.",
        "A price whose log changes by a steady drift and random shocks.",
    ] {
        assert!(relation.contains(expected), "{relation}");
    }

    let progress: Vec<Step> = run.steps.iter().map(|(step, _)| *step).collect();
    let writing = |done| Step::Writing {
        kind: GenerationKind::Synthetic,
        done,
        total: 4,
    };
    assert_eq!(progress, (0..=4).map(writing).collect::<Vec<_>>());
    assert!(
        run.steps[0].1.is_empty(),
        "nothing is paid for before the first draft"
    );
    let last = &run.steps.last().unwrap().1;
    assert_eq!(last, &run.usage);
    assert_eq!(
        (last.tokens().input_tokens, last.tokens().output_tokens),
        (8_000, 800)
    );
}

fn timed_out() -> LlmError {
    LlmError::TimedOut { seconds: 120 }
}

#[tokio::test]
async fn a_draft_the_judge_turns_down_or_the_model_cannot_write_is_counted_and_left_out() {
    let section = in_depth_section();
    let rewrites = [
        (CptKind::RephraseWiki, 1),
        (CptKind::RephraseLecture, 1),
        (CptKind::RephraseQa, 1),
        (CptKind::NotationVariant, 1),
        (CptKind::Exercise, 1),
    ];
    let work = work_of(
        vec![section_plan(&section, &rewrites, vec![assumes(&section)])],
        vec![section.clone()],
    );
    let llm = drafting(|role, input| {
        if role.id == REPHRASE_LECTURE.id {
            return Some(Err(timed_out()));
        }
        if role.id != JUDGE.id {
            return None;
        }
        let verdict = |claims: &[&str], faithful: bool, score: u8| {
            let verdict =
                json!({ "unsupported_claims": claims, "faithful": faithful, "score": score });
            Some(Ok(verdict))
        };
        match input.lines().next().unwrap() {
            "Kind: rephrase_qa" => verdict(&[], false, 4),
            "Kind: exercise" => verdict(&["a rate the section does not give"], true, 5),
            "Kind: notation_variant" => verdict(&[], true, 2),
            "Kind: entigraph" => Some(Err(timed_out())),
            _ => None,
        }
    });

    let (folder, run) = work.run_in_new_folder(&llm).await;

    let records = synthetic_records(folder.path());
    assert_eq!(
        ids_of(&records),
        [format!("{}:rephrase_wiki:1", section.id)],
        "only the draft that the judge passed is written"
    );
    assert_eq!(run.written.by_kind, once_each("rephrase_wiki"));
    assert_eq!(
        run.written.rejected,
        once_each("entigraph exercise notation rephrase_lecture rephrase_qa"),
        "a draft is counted by the role that wrote it, whether the writer failed, the judge failed \
         or the judge turned the text down"
    );
}

#[tokio::test]
async fn a_second_run_over_the_same_plan_asks_nothing_and_writes_the_same_records() {
    let section = in_depth_section();
    let work = work_of(vec![four_texts(&section)], vec![section]);
    let folder = tempfile::tempdir().unwrap();
    let cache = folder.path().join("cache");
    let (first_out, second_out) = (folder.path().join("first"), folder.path().join("second"));
    let (first_llm, second_llm) = (drafting(|_, _| None), drafting(|_, _| None));

    let first = work.run(&first_llm, &cache, &first_out).await;
    let second = work.run(&second_llm, &cache, &second_out).await;

    assert_eq!(
        first.asked, 8,
        "the first run pays for four writers and four judges"
    );
    assert_eq!((second.asked, second_llm.calls()), (0, 0));
    assert_eq!(
        fs::read(first_out.join("cpt/synthetic.jsonl")).unwrap(),
        fs::read(second_out.join("cpt/synthetic.jsonl")).unwrap(),
        "the same ids and the same texts"
    );
    assert_eq!(first.written.by_kind, second.written.by_kind);
    assert_eq!(first.written.rejected, second.written.rejected);
}

// SMELL: this file is close to the most lines one file may hold. The long-section test and the
// helpers that build its text are about how a section is cut, and belong in a file of their own.

/// Four paragraphs of about 250 estimated tokens, in words that no other paragraph shares.
fn prose_block(label: &str) -> String {
    let paragraph = |number: usize| {
        let mut text = format!("This paragraph is {label}{number}.");
        let mut fact = 0;
        while text.len() < 1_000 {
            fact += 1;
            text.push_str(&format!(" It says {label}{number}fact{fact}."));
        }
        text
    };
    (1..=4).map(paragraph).collect::<Vec<_>>().join("\n\n")
}

fn lines(count: usize, line: impl Fn(usize) -> String) -> String {
    (1..=count).map(line).collect::<Vec<_>>().join("\n")
}

/// A display formula with an empty line in the middle of it, and its printed label after it.
fn long_formula() -> String {
    let equation = |number: usize| format!("x_{{{number}}} &= y_{{{number}}} + {number} \\\\");
    let (before, after) = (lines(130, equation), lines(40, equation));
    format!("$$\n{before}\n\n{after}\n$$\n(9.9)")
}

/// A fenced listing with an empty line in the middle of it.
fn long_listing() -> String {
    let assignment = |number: usize| format!("value_{number} = {number} * rate");
    let (before, after) = (lines(160, assignment), lines(70, assignment));
    format!("```python\n{before}\n\n{after}\n```")
}

#[tokio::test]
async fn a_long_section_is_written_in_passages_with_distinct_ids_and_whole_formulas() {
    let mut section = in_depth_section();
    let heading = section.markdown.lines().next().unwrap().to_owned();
    let (formula, listing) = (long_formula(), long_listing());
    let (a, b, c) = (prose_block("a"), prose_block("b"), prose_block("c"));
    section.markdown = format!("{heading}\n\n{a}\n\n{formula}\n\n{b}\n\n{listing}\n\n{c}");
    section.n_tokens = estimated_tokens(&section.markdown);
    assert!(section.n_tokens > 2_500, "{}", section.n_tokens);
    let work = work_of(
        vec![section_plan(
            &section,
            &[(CptKind::RephraseWiki, 2)],
            Vec::new(),
        )],
        vec![section.clone()],
    );
    let llm = drafting(|_, _| None);

    let (folder, _run) = work.run_in_new_folder(&llm).await;

    let inputs: Vec<String> = llm
        .questions()
        .into_iter()
        .filter(|question| question.system_prompt == REPHRASE_WIKI.system_prompt)
        .map(|question| question.input)
        .collect();
    let passages = 3;
    assert_eq!(inputs.len(), 2 * passages, "two versions of each passage");
    let records = synthetic_records(folder.path());
    let expected_ids: Vec<String> = (1..=2 * passages)
        .map(|number| format!("{}:rephrase_wiki:{number}", section.id))
        .collect();
    assert_eq!(ids_of(&records), expected_ids);
    let texts: HashSet<&str> = records
        .iter()
        .map(|record| record["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts.len(), 2 * passages, "every text is its own");

    let count = |input: &str, is_wanted: fn(&str) -> bool| {
        input.lines().filter(|line| is_wanted(line)).count()
    };
    for input in &inputs {
        let source = input.strip_prefix("<source>\n").unwrap();
        assert!(source.starts_with(&heading), "{source:.200}");
        let (body, _notes) = source.split_once("\n\nNotes:\n").unwrap();
        let tokens = estimated_tokens(body);
        assert!(tokens <= 2_500, "{tokens}");
        let formula_fences = count(input, |line| line == "$$");
        assert!(formula_fences.is_multiple_of(2), "a formula is cut in two");
        let listing_fences = count(input, |line| line.starts_with("```"));
        assert!(listing_fences.is_multiple_of(2), "a listing is cut in two");
    }
    for whole in [&formula, &listing] {
        assert!(
            inputs.iter().any(|input| input.contains(whole.as_str())),
            "a passage holds the whole block"
        );
    }
}
