//! Ingests the three committed chapters with their concepts, with the real Gemini API and the real
//! `claude` program, into throwaway stores, and then runs the real `rag-query` command on them.
//! Only that shows that a question reaches the other document through a shared concept, that no
//! golden question is lost, and that Sonnet cites the formula it is given.
//!
//! The run is paid for once. So it prints what each step made as soon as the step ends, and it
//! makes every check and names all that failed together at the end.

use std::path::{Component, Path, PathBuf};

use rag_core::{ClaudeCli, Config, GeminiEmbedder};
use rag_ingestion::testing::ThrowawayStores;
use rag_ingestion::{ConceptExtractor, EXTRACTION_MODEL, Models, ingest_chapter};
use rag_retrieval::read_golden_questions;

use crate::live::{EQUATION_LATEX, EQUATION_QUESTION};
use crate::support::{self, IN_DEPTH_CHAPTER_TITLE, rag_query, workspace_root};

const CACHE_FOLDER_VARIABLE: &str = "RETRIEVAL_LIVE_CACHE_DIR";
const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true RETRIEVAL_LIVE_CACHE_DIR=<an absolute folder outside the workspace> cargo test -p rag-retrieval --test integration -- --ignored live_graph:: --nocapture";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real Gemini API and the real claude program; run it with: {RUN_COMMAND}"
    );
}

/// `claude` is not started while the key is set, so the first question of the run would stop it.
fn require_no_api_key() {
    assert!(
        std::env::var_os("ANTHROPIC_API_KEY").is_none_or(|key| key.is_empty()),
        "ANTHROPIC_API_KEY is set, so claude would not start; unset it and run again"
    );
}

/// The folder that the answers of concept extraction are kept in. With `RETRIEVAL_LIVE_CACHE_DIR`
/// the same answers serve every run, so only the first run pays for them. The folder must never
/// be the real cache, so it is refused before anything is paid for when it is in the workspace
/// (where the default real cache is), or is the real cache wherever `CONCEPT_CACHE_DIR` puts it.
/// It is made only after it has passed both checks, so a folder that is refused is never made.
fn cache_folder(stores: &ThrowawayStores) -> PathBuf {
    let Some(folder) = std::env::var_os(CACHE_FOLDER_VARIABLE).filter(|folder| !folder.is_empty())
    else {
        return stores.config().concept_cache_folder.clone();
    };
    let folder = PathBuf::from(folder);
    assert!(
        folder.is_absolute() && !folder.components().any(|part| part == Component::ParentDir),
        "{CACHE_FOLDER_VARIABLE} must be an absolute path with no `..` in it, and it is {}",
        folder.display()
    );
    let named = with_links_followed(&folder);
    let workspace = std::fs::canonicalize(workspace_root()).expect("the workspace should exist");
    assert!(
        !named.starts_with(&workspace),
        "{CACHE_FOLDER_VARIABLE} is {}, which is inside the workspace {}, where the real cache is",
        named.display(),
        workspace.display()
    );
    let real = Config::load()
        .expect("the settings should load")
        .concept_cache_folder;
    // A relative setting starts at the workspace, and an absolute one replaces it.
    let real = with_links_followed(&workspace.join(real));
    assert!(
        !named.starts_with(&real),
        "{CACHE_FOLDER_VARIABLE} is {}, which is the real cache {}",
        named.display(),
        real.display()
    );
    std::fs::create_dir_all(&named).expect("the cache folder should be made");
    std::fs::canonicalize(&named).expect("the cache folder should exist")
}

/// The folder as the file system knows it, also when it is not there yet: the links of the
/// nearest parent that exists are followed, and the rest of the path is added as it is written.
fn with_links_followed(folder: &Path) -> PathBuf {
    let existing = folder
        .ancestors()
        .find(|parent| parent.exists())
        .expect("the root folder should exist");
    let rest = folder
        .strip_prefix(existing)
        .expect("a parent should be the start of its path");
    let mut followed =
        std::fs::canonicalize(existing).expect("a folder that exists should be found");
    followed.extend(rest);
    followed
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lines_of(output: &std::process::Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the real Gemini API and the real claude program: about 150 haiku questions the first time and almost none again with the same RETRIEVAL_LIVE_CACHE_DIR, and one Sonnet call each run, and spends API credit and subscription usage; run with: set -a; . ./.env; set +a; REX_PROD_API=true RETRIEVAL_LIVE_CACHE_DIR=<an absolute folder outside the workspace> cargo test -p rag-retrieval --test integration -- --ignored live_graph:: --nocapture"]
async fn three_chapters_with_concepts_keep_every_golden_question_span_two_documents_and_answer_with_citations_live()
 {
    require_prod_api();
    require_no_api_key();
    let throwaway = ThrowawayStores::new("live-graph");
    let config = Config {
        concept_cache_folder: cache_folder(&throwaway),
        ..throwaway.config().clone()
    };
    println!(
        "--- the answers of concept extraction are kept in {}",
        config.concept_cache_folder.display()
    );
    let stores = throwaway.connect().await;
    let models = Models {
        embedder: GeminiEmbedder::from_config(&config).expect("the embedder should be set up"),
        concepts: ConceptExtractor::new(ClaudeCli::new(EXTRACTION_MODEL), &config),
    };
    let mut failures: Vec<String> = Vec::new();
    let mut check = |holds: bool, what: String| {
        if !holds {
            failures.push(what);
        }
    };

    let mut mentions_written = 0;
    for chapter in [
        support::intuition_chapter(),
        support::in_depth_chapter(),
        support::sample_chapter(),
    ] {
        let summary = ingest_chapter(&chapter, &models, &stores)
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "could not ingest {}: {:#}",
                    chapter.display(),
                    anyhow::Error::new(error)
                )
            });
        println!("--- ingested {}\n{summary}\n", chapter.display());
        mentions_written += summary.concepts.mentions_written;
    }
    check(
        mentions_written > 0,
        "the chapters were ingested with no concept at all".to_owned(),
    );

    let golden = read_golden_questions(&workspace_root().join("golden.toml")).unwrap();
    let evaluated = rag_query(&throwaway, &["eval"]);
    check(
        evaluated.status.success(),
        format!(
            "eval failed: {}",
            String::from_utf8_lossy(&evaluated.stderr)
        ),
    );
    let printed = lines_of(&evaluated);
    for question in &golden {
        let ending = format!(": {}", one_line(&question.text));
        check(
            printed
                .iter()
                .any(|line| line.starts_with("found at ") && line.ends_with(&ending)),
            format!("not found in the top 5: {}", question.text),
        );
    }
    let score = format!("found in the top 5: {0} of {0}", golden.len());
    check(
        printed.last() == Some(&score),
        format!("the last line is not `{score}`: {:?}", printed.last()),
    );

    let spanning = golden
        .iter()
        .find(|question| !question.also.is_empty())
        .expect("golden.toml has a question whose answer is in two documents");
    let asked = rag_query(&throwaway, &[spanning.text.as_str()]);
    check(
        asked.status.success(),
        format!(
            "the question failed: {}",
            String::from_utf8_lossy(&asked.stderr)
        ),
    );
    let asked_lines = lines_of(&asked);
    let places =
        std::iter::once(&spanning.document).chain(spanning.also.iter().map(|p| &p.document));
    for title in places {
        check(
            asked_lines.contains(&format!("document: {title}")),
            format!("no result is from {title}"),
        );
    }

    let answered = rag_query(&throwaway, &["--answer", EQUATION_QUESTION]);
    check(
        answered.status.success(),
        format!(
            "--answer failed: {}",
            String::from_utf8_lossy(&answered.stderr)
        ),
    );
    let answer = String::from_utf8_lossy(&answered.stdout).into_owned();
    check(
        answer
            .lines()
            .any(|line| line.starts_with(&format!("  source: {IN_DEPTH_CHAPTER_TITLE}, page 7"))),
        "no source line names page 7 of the in-depth chapter".to_owned(),
    );
    check(
        answer.contains(EQUATION_LATEX),
        "the LaTeX of the equation is not printed unchanged".to_owned(),
    );

    assert!(
        failures.is_empty(),
        "{} checks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
