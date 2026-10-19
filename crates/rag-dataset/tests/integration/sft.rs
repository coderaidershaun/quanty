//! The fine-tuning examples of a dataset: questions asked of the sections, answers solved and
//! checked, and the three files of records, through `write_sft` with a model that gives the
//! replies the test scripts and a real `python3` for the programs.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ocr::read_chapter;
use rag_core::LlmError;
use rag_dataset::documents::Split;
use rag_dataset::generate::asker::Asker;
use rag_dataset::plan::{GenerationPlan, SectionPlan};
use rag_dataset::section::{Section, sections_of};
use rag_dataset::sft::{JUDGE_ANSWER, QUESTIONS, SOLVE_DIRECT, SOLVE_THINK, write_sft};
use rag_dataset::{BuildOptions, DatasetFolder, GenerationKind, Step, Written};
use rag_ingestion::chapter_items;
use rag_ingestion::testing::{AskedQuestion, StandInLlm};
use serde_json::{Value, json};

use crate::support;

const TRAIN_HEADING: &str = "# Quanty Sample Notes › Black Scholes In Depth";
const HELDOUT_HEADING: &str = "# Quanty Sample Notes › Options Pricing Intuition";
const TRAIN_SECTION: &str = "quanty-sample-notes:chapter-2:s001";
const HELDOUT_SECTION: &str = "quanty-sample-notes:chapter-1:s001";

/// The one section of a hand-written chapter, on this side of the held-out line.
fn section_of(chapter_folder: &Path, split: Split) -> Section {
    let mut document = support::exported_at(&support::sample_content(), chapter_folder);
    document.split = split;
    let chapter = read_chapter(chapter_folder).unwrap();
    let mut sections = sections_of(&document, &chapter, &chapter_items(&chapter));
    assert_eq!(
        sections.len(),
        1,
        "the chapter should be cut in one section"
    );
    sections.remove(0)
}

fn train_section() -> Section {
    section_of(&support::in_depth_chapter(), Split::Train)
}

fn heldout_section() -> Section {
    section_of(&support::intuition_chapter(), Split::Heldout)
}

/// The options of a build into `out`, with three tries at each question to think about.
fn options_into(out: &Path) -> BuildOptions {
    BuildOptions {
        samples: 3,
        ..support::options_over(&support::sample_content(), out)
    }
}

/// A plan by hand that asks this many questions of each section.
fn plan_of(questions: &[(&Section, u32)]) -> GenerationPlan {
    GenerationPlan {
        sections: questions
            .iter()
            .map(|(section, questions)| SectionPlan {
                section_id: section.id.clone(),
                questions: *questions,
                ..SectionPlan::default()
            })
            .collect(),
        ..GenerationPlan::default()
    }
}

// SMELL: this file is close to the 500-line limit for one file. The scripted model below is the
// part to move to a file of its own when another test needs room.
/// What a question to the stand-in model asks for. The questions that a test scripts start with a
/// marker such as `[q3]`, which no book holds, and every solver and judge message holds the whole
/// section, so only the marker and the last line tell them apart.
enum Job<'a> {
    /// The questions for the section under this heading.
    Questions(&'a str),
    /// A try, by sample number, at the question with this marker.
    Think(&'a str, u32),
    Direct(&'a str),
    Judge(&'a str),
}

fn job_of(question: &AskedQuestion) -> Job<'_> {
    let input = question.input.as_str();
    if question.system_prompt == QUESTIONS.system_prompt {
        let heading = if input.contains(HELDOUT_HEADING) {
            HELDOUT_HEADING
        } else {
            TRAIN_HEADING
        };
        return Job::Questions(heading);
    }
    let start = input
        .find("Question:\n[q")
        .map(|found| found + "Question:\n".len())
        .expect("the question should start with its marker");
    let marker = &input[start..start + "[q1]".len()];
    if question.system_prompt == JUDGE_ANSWER.system_prompt {
        Job::Judge(marker)
    } else if question.system_prompt == SOLVE_DIRECT.system_prompt {
        Job::Direct(marker)
    } else {
        assert_eq!(question.system_prompt, SOLVE_THINK.system_prompt);
        let sample = input
            .rsplit_once("\n\nSample ")
            .and_then(|(_, number)| number.parse().ok())
            .expect("a question to think ends with its sample line");
        Job::Think(marker, sample)
    }
}

fn written(question: &str, task: &str, seed_kind: &str) -> Value {
    json!({
        "question": question,
        "task": task,
        "difficulty": 0.4,
        "seed": { "kind": seed_kind, "quote": "the sentence the question rests on" }
    })
}

/// The reply of a solver that thinks. The working of the second sample is the shortest.
fn thought(sample: u32, answer: &str, final_value: &str) -> Value {
    let working = "w".repeat([30, 10, 20][sample as usize - 1]);
    json!({
        "thinking": format!("sample {sample}: {working}"),
        "answer": answer,
        "final_value": final_value
    })
}

fn approved() -> Value {
    json!({ "faithful": true, "unsupported_claims": [], "score": 5 })
}

/// The `verification` of a record as the files spell it.
fn verification(
    method: &str,
    passed: bool,
    n_teacher_samples: u32,
    agreement: u32,
    judge_score: Option<u8>,
) -> Value {
    json!({
        "method": method,
        "passed": passed,
        "n_teacher_samples": n_teacher_samples,
        "agreement": agreement,
        "judge_score": judge_score
    })
}

fn counts(pairs: &[(&str, usize)]) -> BTreeMap<String, usize> {
    pairs
        .iter()
        .map(|(name, count)| ((*name).to_owned(), *count))
        .collect()
}

struct Run {
    written: Written,
    steps: Vec<Step>,
    asked: usize,
}

/// Writes the examples of the sections into `options.out`, asking `llm` through a new asker over
/// the cache folder of the options.
async fn write_in(
    llm: &StandInLlm,
    plan: &GenerationPlan,
    sections: &[Section],
    options: &BuildOptions,
) -> Run {
    let asker = Asker::new(llm, &options.cache_folder).unwrap();
    let mut steps = Vec::new();
    let written = write_sft(
        plan,
        sections,
        &asker,
        &DatasetFolder::new(&options.out),
        options,
        &mut |step, _| steps.push(step),
    )
    .await
    .unwrap();
    Run {
        written,
        steps,
        asked: asker.calls(),
    }
}

/// Three questions for the train section (a calculation, an explanation and a program) and one
/// explanation for the held-out section. The three solvers agree on 10.45, the program prints
/// it, and the judge approves of everything.
fn model_that_agrees() -> StandInLlm {
    StandInLlm::replying_to_questions(|question, _| {
        Ok(match job_of(question) {
            Job::Questions(TRAIN_HEADING) => json!({ "questions": [
                written("[q1] What is the price of the call?", "calculation", "book_worked_example"),
                written("[q2] Why does the model need a risk-free rate?", "explain", "generated"),
                written("[q3] Compute the price of the call with a program.", "code_pot", "book_exercise"),
            ] }),
            Job::Questions(_) => json!({ "questions": [
                written("[q4] What does an option give its holder?", "explain", "generated"),
            ] }),
            Job::Think("[q1]", sample) => thought(sample, "The price is $10.45$.", "10.45"),
            Job::Think("[q3]", sample) => thought(sample, "```python\nprint(10.45)\n```", "10.45"),
            Job::Direct(_) => json!({ "answer": "Because money has a time value." }),
            Job::Judge(_) => approved(),
            Job::Think(marker, _) => panic!("no script for {marker}"),
        })
    })
}

#[tokio::test]
async fn verified_answers_become_think_direct_and_code_examples_and_a_second_run_asks_nothing() {
    let work = tempfile::tempdir().unwrap();
    let sections = [train_section(), heldout_section()];
    let plan = plan_of(&[(&sections[0], 3), (&sections[1], 1)]);
    let options = options_into(&work.path().join("first"));
    let llm = model_that_agrees();

    let first = write_in(&llm, &plan, &sections, &options).await;

    let train = support::read_jsonl(&options.out.join("sft/train.jsonl"));
    let ids: Vec<&str> = train
        .iter()
        .map(|record| record["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            format!("sft:{TRAIN_SECTION}:0001"),
            format!("sft:{TRAIN_SECTION}:0002"),
            format!("sft:{TRAIN_SECTION}:0003"),
        ]
    );
    let [calculation, explanation, program] = &train[..] else {
        panic!("three examples should be written for training: {train:?}");
    };

    assert_eq!(calculation["task"], "calculation");
    assert_eq!(calculation["reasoning"], "think");
    assert_eq!(calculation["split"], "train");
    assert_eq!(calculation["seed"]["kind"], "book_worked_example");
    assert_eq!(
        calculation["messages"][0],
        json!({ "role": "user", "content": "[q1] What is the price of the call?" })
    );
    let answer = &calculation["messages"][1];
    assert_eq!(answer["role"], "assistant");
    assert_eq!(answer["content"], "The price is $10.45$.");
    assert!(
        answer["thinking"]
            .as_str()
            .unwrap()
            .starts_with("sample 2:"),
        "the example keeps the shortest of the three samples: {answer}"
    );
    assert_eq!(
        calculation["verification"],
        verification("agreement+judge", true, 3, 3, Some(5))
    );

    assert_eq!(explanation["reasoning"], "direct");
    let straight = explanation["messages"][1].as_object().unwrap();
    assert_eq!(straight["content"], "Because money has a time value.");
    assert!(
        !straight.contains_key("thinking"),
        "a direct answer has no thinking key: {straight:?}"
    );
    assert_eq!(
        explanation["verification"],
        verification("judge", true, 1, 1, Some(5))
    );

    assert_eq!(program["task"], "code_pot");
    assert!(
        program["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("print(10.45)")
    );
    assert_eq!(
        program["verification"],
        verification("python_exec", true, 3, 3, None)
    );

    let heldout = support::read_jsonl(&options.out.join("sft/heldout.jsonl"));
    assert_eq!(heldout.len(), 1);
    assert_eq!(heldout[0]["id"], format!("sft:{HELDOUT_SECTION}:0001"));
    assert_eq!(heldout[0]["split"], "heldout");
    assert_eq!(
        fs::read_to_string(options.out.join("sft/rejected.jsonl")).unwrap(),
        "",
        "nothing was rejected, and the file is written all the same"
    );

    assert_eq!(
        first.written.by_kind,
        counts(&[("calculation", 1), ("explain", 2), ("code_pot", 1)])
    );
    assert_eq!(
        first.written.by_reasoning,
        counts(&[("think", 2), ("direct", 2)])
    );
    assert!(first.written.rejected.is_empty());
    let progress = |done| Step::Writing {
        kind: GenerationKind::Sft,
        done,
        total: 4,
    };
    assert_eq!(first.steps, [progress(0), progress(4)]);
    assert!(llm.calls() > 0);

    let mut again = options.clone();
    again.out = work.path().join("second");
    let second_llm = model_that_agrees();

    let second = write_in(&second_llm, &plan, &sections, &again).await;

    assert_eq!(second.asked, 0, "every answer was kept the first time");
    assert_eq!(second_llm.calls(), 0);
    assert_eq!(second.written.by_kind, first.written.by_kind);
    for file in ["train", "heldout", "rejected"] {
        let name = format!("sft/{file}.jsonl");
        assert_eq!(
            fs::read(again.out.join(&name)).unwrap(),
            fs::read(options.out.join(&name)).unwrap(),
            "{name} is the same on the second run"
        );
    }
}

/// Five questions for the train section, each of which ends differently:
/// - [q1] tries 10.45, 10.45 and 12.0, which is a majority, and the judge approves;
/// - [q2] tries 10.45, 11.0 and 12.0, which is no majority;
/// - [q3] is an explanation that the judge finds unfaithful;
/// - [q4] is a program that exits with an error;
/// - [q5] is a question that the solvers never answer.
fn model_that_gets_some_wrong() -> StandInLlm {
    let priced =
        |sample: u32, values: [&str; 3]| thought(sample, "Priced.", values[sample as usize - 1]);
    StandInLlm::replying_to_questions(move |question, _| {
        Ok(match job_of(question) {
            Job::Questions(_) => json!({ "questions": [
                written("[q1] Price the call.", "calculation", "generated"),
                written("[q2] Price the put.", "calculation", "generated"),
                written("[q3] Why is the volatility a constant?", "explain", "generated"),
                written("[q4] Price the call with a program.", "code_pot", "generated"),
                written("[q5] Price the straddle.", "calculation", "generated"),
            ] }),
            Job::Think("[q1]", sample) => priced(sample, ["10.45", "10.45", "12.0"]),
            Job::Think("[q2]", sample) => priced(sample, ["10.45", "11.0", "12.0"]),
            Job::Think("[q4]", sample) => {
                thought(sample, "```python\nimport sys; sys.exit(1)\n```", "10.45")
            }
            Job::Think("[q5]", _) => return Err(LlmError::TimedOut { seconds: 120 }),
            Job::Direct("[q3]") => json!({ "answer": "Because it is simpler." }),
            Job::Judge("[q1]") => approved(),
            Job::Judge("[q3]") => json!({
                "faithful": false,
                "unsupported_claims": ["the section never says so"],
                "score": 2
            }),
            Job::Think(marker, _) | Job::Direct(marker) | Job::Judge(marker) => {
                panic!("nothing is scripted for {marker}, or it should not be asked")
            }
        })
    })
}

#[tokio::test]
async fn answers_that_fail_their_checks_go_to_rejected_with_the_reason() {
    let work = tempfile::tempdir().unwrap();
    let section = train_section();
    let plan = plan_of(&[(&section, 5)]);
    let options = options_into(work.path());

    let run = write_in(&model_that_gets_some_wrong(), &plan, &[section], &options).await;

    let train = support::read_jsonl(&options.out.join("sft/train.jsonl"));
    assert_eq!(train.len(), 1, "only the first question passed: {train:?}");
    assert_eq!(train[0]["id"], format!("sft:{TRAIN_SECTION}:0001"));
    assert_eq!(
        train[0]["verification"],
        verification("agreement+judge", true, 3, 2, Some(5))
    );

    let rejected = support::read_jsonl(&options.out.join("sft/rejected.jsonl"));
    let reasons: Vec<(&str, &str)> = rejected
        .iter()
        .map(|record| {
            (
                record["id"].as_str().unwrap().rsplit(':').next().unwrap(),
                record["rejected"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        reasons,
        [
            ("0002", "disagreement"),
            ("0003", "judge"),
            ("0004", "python_exec"),
            ("0005", "no_answer"),
        ]
    );
    assert_eq!(
        rejected[0]["verification"],
        verification("agreement+judge", false, 3, 1, None)
    );
    assert_eq!(
        rejected[1]["verification"],
        verification("judge", false, 1, 0, Some(2))
    );
    assert_eq!(
        rejected[2]["verification"],
        verification("python_exec", false, 3, 0, None)
    );
    assert_eq!(rejected[3]["verification"]["passed"], false);
    assert_eq!(
        rejected[0]["messages"][1]["content"], "Priced.",
        "a rejected example keeps the best answer"
    );
    assert_eq!(
        rejected[3]["messages"].as_array().unwrap().len(),
        1,
        "no answer came back, so only the question is kept"
    );

    assert_eq!(run.written.by_kind, counts(&[("calculation", 1)]));
    assert_eq!(run.written.by_reasoning, counts(&[("think", 1)]));
    assert_eq!(
        run.written.rejected,
        counts(&[
            ("disagreement", 1),
            ("judge", 1),
            ("python_exec", 1),
            ("no_answer", 1)
        ])
    );
}

/// One code question whose program leaves a file at `marker` when it runs.
fn model_that_writes_a_file(marker: &Path) -> StandInLlm {
    let program = format!("```python\nopen({marker:?}, 'w').close()\nprint(10.45)\n```");
    StandInLlm::replying_to_questions(move |question, _| {
        Ok(match job_of(question) {
            Job::Questions(_) => json!({ "questions": [
                written("[q1] Price the call with a program.", "code_pot", "generated"),
            ] }),
            Job::Think(_, sample) => thought(sample, &program, "10.45"),
            Job::Judge(_) => approved(),
            Job::Direct(marker) => panic!("{marker} is not an explanation"),
        })
    })
}

#[tokio::test]
async fn without_run_code_a_code_question_is_judged_and_its_program_never_runs() {
    let work = tempfile::tempdir().unwrap();
    let marker = work.path().join("the-program-ran");
    let section = train_section();
    let plan = plan_of(&[(&section, 1)]);
    let sections = [section];
    let mut options = options_into(&work.path().join("out"));
    options.run_code = false;
    let llm = model_that_writes_a_file(&marker);

    write_in(&llm, &plan, &sections, &options).await;

    let train = support::read_jsonl(&options.out.join("sft/train.jsonl"));
    assert_eq!(
        train[0]["verification"],
        verification("judge", true, 3, 1, Some(5))
    );
    assert!(
        llm.questions()
            .iter()
            .any(|question| question.system_prompt == JUDGE_ANSWER.system_prompt),
        "the judge was asked"
    );
    assert!(!marker.exists(), "the program must not have run");

    options.run_code = true;
    options.out = work.path().join("control");
    write_in(&llm, &plan, &sections, &options).await;
    assert!(
        marker.exists(),
        "with run_code the same program runs, so the marker proves it did not before"
    );
}
