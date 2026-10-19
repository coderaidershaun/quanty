//! One build with every stage on, and the texts the plan counts for a section of several passages.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ocr::{read_chapter, sha256_hex};
use rag_core::{LlmError, UsageTally};
use rag_dataset::generate::role::Role;
use rag_dataset::generate::{
    ENTIGRAPH, EXERCISE, JUDGE, NOTATION, REPHRASE_LECTURE, REPHRASE_QA, REPHRASE_WIKI,
};
use rag_dataset::plan::plan_for;
use rag_dataset::sample::CptKind;
use rag_dataset::section::{Section, estimated_tokens, sections_of};
use rag_dataset::sft::{JUDGE_ANSWER, QUESTIONS, SOLVE_DIRECT, SOLVE_THINK};
use rag_dataset::{BuildOptions, DatasetFolder, Report, Step, build};
use rag_ingestion::chapter_items;
use rag_ingestion::testing::{AskedQuestion, StandInLlm};
use serde_json::{Value, json};

use crate::support;

/// The roles that write a text, which the stand-in answers with a text made from the input.
const WRITERS: [&Role; 6] = [
    &REPHRASE_WIKI,
    &REPHRASE_LECTURE,
    &REPHRASE_QA,
    &ENTIGRAPH,
    &EXERCISE,
    &NOTATION,
];

/// Every role a build asks.
const ROLES: [&Role; 11] = [
    &REPHRASE_WIKI,
    &REPHRASE_LECTURE,
    &REPHRASE_QA,
    &ENTIGRAPH,
    &EXERCISE,
    &NOTATION,
    &JUDGE,
    &QUESTIONS,
    &SOLVE_THINK,
    &SOLVE_DIRECT,
    &JUDGE_ANSWER,
];

fn written_question(question: &str, task: &str, seed_kind: &str) -> Value {
    json!({
        "question": question,
        "task": task,
        "difficulty": 0.4,
        "seed": { "kind": seed_kind, "quote": "the sentence the question rests on" }
    })
}

/// The marker, such as `[q1]`, that starts the question an answer or a judgement is asked about.
fn marker_of(input: &str) -> &str {
    let start = input
        .find("Question:\n[q")
        .map(|found| found + "Question:\n".len())
        .expect("the question should start with its marker");
    &input[start..start + "[q1]".len()]
}

/// What a model that answers every role says. Each writer gives a text of its own for each input.
/// The judge of texts gives a 4 to every text but the exercises, which it gives a 2 and so turns
/// down. Four questions are written from every section: a calculation and a program that the
/// solvers agree on, an explanation, and a comparison that the judge of answers turns down.
fn answer_to(question: &AskedQuestion) -> Result<Value, LlmError> {
    let prompt = question.system_prompt.as_str();
    let input = question.input.as_str();
    if let Some(writer) = WRITERS.iter().find(|role| role.system_prompt == prompt) {
        let fingerprint = sha256_hex(input.as_bytes());
        return Ok(json!({ "text": format!("{} text {}", writer.id, &fingerprint[..16]) }));
    }
    Ok(if prompt == JUDGE.system_prompt {
        let score = if input.starts_with("Kind: exercise") {
            2
        } else {
            4
        };
        json!({ "faithful": true, "unsupported_claims": [], "score": score })
    } else if prompt == QUESTIONS.system_prompt {
        json!({ "questions": [
            written_question("[q1] What is the price of the call?", "calculation", "book_worked_example"),
            written_question("[q2] Why does the model need a risk-free rate?", "explain", "generated"),
            written_question("[q3] Compute the price of the call with a program.", "code_pot", "book_exercise"),
            written_question("[q4] How do the two prices differ?", "compare", "generated"),
        ] })
    } else if prompt == SOLVE_THINK.system_prompt {
        match marker_of(input) {
            "[q1]" => json!({
                "thinking": "work", "answer": "The price is $10.45$.", "final_value": "10.45"
            }),
            "[q3]" => json!({
                "thinking": "work", "answer": "```python\nprint(10.45)\n```", "final_value": "10.45"
            }),
            marker => panic!("no script for {marker}"),
        }
    } else if prompt == SOLVE_DIRECT.system_prompt {
        json!({ "answer": "Because money has a time value." })
    } else if prompt == JUDGE_ANSWER.system_prompt {
        let score = if marker_of(input) == "[q4]" { 2 } else { 5 };
        json!({ "faithful": true, "unsupported_claims": [], "score": score })
    } else {
        panic!("no role has this prompt: {prompt}")
    })
}

/// The one section of the second hand-written chapter, with the text of two paragraphs of about
/// 1,400 tokens under its heading line. That is over the 2,500 tokens that a writer reads at a
/// time, so the writers read it in two passages. It holds no display formula, so it gets no
/// notation variant.
fn section_of_two_passages() -> Section {
    let document = support::exported_at(&support::sample_content(), &support::in_depth_chapter());
    let chapter = read_chapter(&document.folder).unwrap();
    let section = sections_of(&document, &chapter, &chapter_items(&chapter)).remove(0);
    let heading_line = section.markdown.lines().next().unwrap();
    let paragraph = |name: &str| format!("{name} {}", "abc ".repeat(1_399).trim_end());
    let markdown = format!(
        "{heading_line}\n\n{}\n\n{}",
        paragraph("FIRST"),
        paragraph("SECOND")
    );
    Section {
        n_tokens: estimated_tokens(&markdown),
        markdown,
        ..section
    }
}

#[tokio::test]
async fn a_long_section_plans_a_text_for_each_passage_and_the_cap_counts_them() {
    let section = section_of_two_passages();
    let mut options = support::options_over(&support::sample_content(), Path::new("unused"));
    options.rewrites = 1;

    let plan = plan_for(std::slice::from_ref(&section), None, &options)
        .await
        .unwrap();

    let planned = &plan.sections[0];
    assert_eq!(planned.passages, 2);
    assert_eq!(
        planned.rewrites[&CptKind::RephraseWiki],
        1,
        "for each passage"
    );
    let texts = &plan.totals.texts;
    assert_eq!(
        *texts,
        [(CptKind::RephraseWiki, 2), (CptKind::Exercise, 2)].into()
    );
    let calls_of = |texts: u32, questions: u32| 2 * texts + (options.samples + 2) * questions;
    assert_eq!(plan.totals.model_calls, calls_of(4, plan.totals.questions));

    options.max_synthetic = Some(3);
    let capped = plan_for(std::slice::from_ref(&section), None, &options)
        .await
        .unwrap();

    // The exercise cannot be written from one passage only, so it is what the cap leaves out.
    assert_eq!(capped.totals.texts, [(CptKind::RephraseWiki, 2)].into());
    assert_eq!(
        capped.totals.model_calls,
        calls_of(2, capped.totals.questions)
    );
}

fn lines_of(file: &Path) -> usize {
    fs::read_to_string(file).unwrap().lines().count()
}

fn read_manifest(folder: &DatasetFolder) -> Value {
    serde_json::from_str(&fs::read_to_string(folder.manifest()).unwrap()).unwrap()
}

/// How often each score from 1 to 5 is given, the way the manifest spells it.
fn histogram(scores: impl Iterator<Item = u64>) -> Value {
    let mut counts: BTreeMap<String, u64> = (1..=5).map(|score| (score.to_string(), 0)).collect();
    for score in scores {
        *counts
            .get_mut(&score.to_string())
            .expect("a score should be from 1 to 5") += 1;
    }
    json!(counts)
}

/// The counts of a report that a second build must give again.
fn counts_of(report: &Report) -> impl PartialEq + std::fmt::Debug + '_ {
    (
        &report.sections,
        &report.rejected,
        &report.planned,
        &report.synthetic.by_kind,
        &report.synthetic.rejected,
        &report.sft.by_kind,
        &report.sft.by_reasoning,
        &report.sft.rejected,
    )
}

fn reasoning_count(report: &Report, name: &str) -> usize {
    report.sft.by_reasoning.get(name).copied().unwrap_or(0)
}

fn assert_every_file_is_written(folder: &DatasetFolder) {
    for file in folder
        .jsonl_files()
        .into_iter()
        .chain([folder.plan(), folder.manifest()])
    {
        assert!(file.is_file(), "{} should be written", file.display());
    }
}

/// The manifest and the report count the lines that the files hold.
fn assert_the_counts_are_the_lines_of_the_files(manifest: &Value, report: &Report, out: &Path) {
    let listed = manifest["files"].as_array().unwrap();
    assert_eq!(listed.len(), 7);
    for file in listed {
        let path = out.join(file["path"].as_str().unwrap());
        assert_eq!(file["samples"], lines_of(&path), "{}", path.display());
    }

    let lines = |file: &str| lines_of(&out.join(file));
    assert_eq!(report.sections.train, lines("cpt/raw.jsonl"));
    assert_eq!(report.sections.heldout, lines("cpt/heldout.jsonl"));
    assert_eq!(report.sections.heldout, 1);
    assert_eq!(
        report.rejected.values().sum::<usize>(),
        lines("cpt/rejected.jsonl")
    );
    assert_eq!(report.synthetic.total(), lines("cpt/synthetic.jsonl"));
    assert_eq!(
        report.sft.total(),
        lines("sft/train.jsonl") + lines("sft/heldout.jsonl")
    );
    assert_eq!(
        report.sft.rejected.values().sum::<usize>(),
        lines("sft/rejected.jsonl")
    );
}

fn assert_the_plan_counts_what_the_writers_were_asked(report: &Report) {
    // The sample book is cut in two passages, so a plan that counts one text for each section
    // would be short.
    assert_eq!(
        report.planned.texts.values().sum::<u32>() as usize,
        report.synthetic.total() + report.synthetic.rejected.values().sum::<usize>()
    );
    assert_eq!(
        report.planned.questions as usize,
        report.sft.total() + report.sft.rejected.values().sum::<usize>()
    );
}

fn assert_the_manifest_counts_what_the_report_counts(manifest: &Value, report: &Report) {
    let think = reasoning_count(report, "think");
    let direct = reasoning_count(report, "direct");
    assert!(think > 0 && direct > 0);
    assert_eq!(
        manifest["mix"],
        json!({
            "raw": report.sections.train,
            "heldout": report.sections.heldout,
            "synthetic_by_kind": report.synthetic.by_kind,
            "sft_by_task": report.sft.by_kind,
            "think": think,
            "direct": direct,
        })
    );
    assert_eq!(
        manifest["rejected"],
        json!({
            "sections": report.rejected,
            "synthetic": report.synthetic.rejected,
            "sft": report.sft.rejected,
        })
    );
    assert!(manifest["rejected"]["synthetic"]["exercise"].as_u64() > Some(0));
    assert!(manifest["rejected"]["sft"]["judge"].as_u64() > Some(0));
}

fn assert_the_judge_scores_are_those_of_the_records(manifest: &Value, out: &Path) {
    let records = |file: &str| support::read_jsonl(&out.join(file));
    let synthetic_scores = records("cpt/synthetic.jsonl")
        .into_iter()
        .filter_map(|record| record["quality"]["judge_score"].as_u64())
        .collect::<Vec<_>>();
    assert_eq!(
        manifest["judge_scores"]["synthetic"],
        histogram(synthetic_scores.into_iter())
    );
    assert_eq!(
        manifest["judge_scores"]["synthetic"]["2"], 0,
        "a draft that was turned down has no record to count"
    );
    let sft_scores = ["sft/train.jsonl", "sft/heldout.jsonl", "sft/rejected.jsonl"]
        .into_iter()
        .flat_map(records)
        .filter_map(|record| record["verification"]["judge_score"].as_u64())
        .collect::<Vec<_>>();
    assert_eq!(
        manifest["judge_scores"]["sft"],
        histogram(sft_scores.into_iter())
    );
}

fn assert_the_manifest_names_its_prompts_and_its_version(manifest: &Value) {
    let versions: Vec<&str> = manifest["prompt_versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|version| version.as_str().unwrap())
        .collect();
    assert_eq!(versions.len(), ROLES.len());
    for role in ROLES {
        assert!(versions.contains(&role.prompt_id().as_str()), "{}", role.id);
    }
    let notes = manifest["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|note| note.as_str().unwrap().contains("docs/training.md"))
    );
    let dataset_version = manifest["sha256"].as_str().unwrap();
    assert!(dataset_version.len() == 64 && dataset_version.chars().all(|c| c.is_ascii_hexdigit()));
}

/// Every synthetic text names the prompt that wrote it, and the held-out file holds the examples
/// of the held-out document alone.
fn assert_the_records_say_where_they_came_from(out: &Path) {
    for record in support::read_jsonl(&out.join("cpt/synthetic.jsonl")) {
        let prompt_id = record["gen"]["prompt_id"].as_str().unwrap();
        let (role, version) = prompt_id.split_once('@').expect("<role>@<version>");
        assert!(
            !role.is_empty() && version.parse::<u32>().is_ok(),
            "{prompt_id}"
        );
        assert_eq!(record["quality"]["judge_score"], 4);
    }
    let held_out_examples = support::read_jsonl(&out.join("sft/heldout.jsonl"));
    assert!(!held_out_examples.is_empty());
    for record in &held_out_examples {
        assert_eq!(record["split"], "heldout");
        assert!(
            record["id"]
                .as_str()
                .unwrap()
                .starts_with("sft:quanty-sample-notes:chapter-1:"),
            "{record}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_full_build_writes_every_file_and_a_second_build_asks_nothing() {
    let work = tempfile::tempdir().unwrap();
    let mut options = support::options_over(&support::sample_content(), &work.path().join("one"));
    options.cache_folder = work.path().join("cache");
    options.questions = 4;
    options.selection.holdout =
        vec!["Quanty Sample Notes, chapter 1: Options Pricing Intuition".to_owned()];
    let llm = StandInLlm::replying_to_questions(|question, _| answer_to(question));
    let mut steps: Vec<(Step, UsageTally)> = Vec::new();

    let report = build(&options, &llm, None, &mut |step, usage| {
        steps.push((step, usage.clone()));
    })
    .await
    .expect("the build should finish");

    let folder = DatasetFolder::new(&options.out);
    assert_every_file_is_written(&folder);
    let manifest = read_manifest(&folder);
    assert_the_counts_are_the_lines_of_the_files(&manifest, &report, &options.out);
    assert_the_plan_counts_what_the_writers_were_asked(&report);
    assert_the_manifest_counts_what_the_report_counts(&manifest, &report);
    assert_the_judge_scores_are_those_of_the_records(&manifest, &options.out);
    assert_the_manifest_names_its_prompts_and_its_version(&manifest);
    assert_the_records_say_where_they_came_from(&options.out);
    let last_writing = steps
        .iter()
        .rev()
        .find(|(step, _)| matches!(step, Step::Writing { .. }))
        .expect("the build should tell its writing steps");
    assert!(!last_writing.1.is_empty(), "the usage reaches the caller");

    let asked = llm.calls();
    let again = BuildOptions {
        out: work.path().join("two"),
        ..options.clone()
    };

    let second = build(&again, &llm, None, &mut |_, _| {})
        .await
        .expect("the second build should finish");

    assert_eq!(llm.calls(), asked, "every answer was kept the first time");
    assert_eq!(counts_of(&second), counts_of(&report));
    assert_eq!(
        read_manifest(&DatasetFolder::new(&again.out))["sha256"],
        manifest["sha256"],
        "the same files make the same version, whenever they were built"
    );
}
