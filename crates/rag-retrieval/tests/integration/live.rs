//! Asks the three committed chapters questions with the real Gemini embedder, once in this
//! process and once through the real `rag-query` command, in a throwaway collection of the local
//! Qdrant. Only real embeddings show that a question lands near the item that answers it, and
//! only the real command shows that the key, the collection and the printed lines work together.

use std::process::{Command, Output};

use rag_core::{Config, GeminiEmbedder, ItemKind, ItemStore};
use rag_retrieval::{Retriever, read_golden_questions};

use crate::support::{
    self, IN_DEPTH_CHAPTER_TITLE, SAMPLE_CHAPTER_TITLE, ThrowawayCollection, store_samples,
    workspace_root,
};

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-retrieval --test integration -- --ignored live:: --nocapture";

const EQUATION_QUESTION: &str = "What is the Black–Scholes partial differential equation?";
const CHART_QUESTION: &str = "Which chart compares the theoretical profit or loss of a short straddle, a ratio spread and a long butterfly?";
const EQUATION_LATEX: &str = r"\frac{\partial V}{\partial t} + \frac{1}{2}\sigma^2 S^2 \frac{\partial^2 V}{\partial S^2} + r S \frac{\partial V}{\partial S} - r V = 0";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real Gemini API; run it with: {RUN_COMMAND}"
    );
}

/// Runs the real command from the workspace root, against the throwaway collection, and prints
/// what it answered, so that one paid run shows everything.
fn run_rag_query(config: &Config, arguments: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_rag-query"))
        .args(arguments)
        .current_dir(workspace_root())
        .env("QDRANT_ITEMS_COLLECTION", &config.items_collection)
        .output()
        .expect("the rag-query binary should start");
    println!("--- rag-query {arguments:?}: {}", output.status);
    println!("{}", String::from_utf8_lossy(&output.stdout));
    println!("--- its standard error:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    output
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the real Gemini API and spends API credit; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-retrieval --test integration -- --ignored live:: --nocapture"]
async fn sample_chapters_answer_a_formula_and_a_chart_question_and_eval_prints_its_score_live() {
    require_prod_api();
    let throwaway = ThrowawayCollection::new("search-live");
    let config = throwaway.config();
    let embedder = GeminiEmbedder::from_config(config).unwrap();
    let store = ItemStore::connect(config).unwrap();
    store_samples(&embedder, &store).await;
    let retriever = Retriever::new(embedder, store);

    let equation = retriever.search(EQUATION_QUESTION, None, 5).await.unwrap();
    println!("--- {EQUATION_QUESTION}\n{equation}\n");
    let chart = retriever.search(CHART_QUESTION, None, 5).await.unwrap();
    println!("--- {CHART_QUESTION}\n{chart}\n");
    let asked = run_rag_query(config, &[EQUATION_QUESTION]);
    let evaluated = run_rag_query(config, &["eval"]);

    assert!(
        equation.hits.iter().any(|hit| {
            hit.payload.kind == ItemKind::Formula
                && hit.payload.doc_title == IN_DEPTH_CHAPTER_TITLE
                && hit.payload.text == EQUATION_LATEX
        }),
        "the equation is not among the results:\n{equation}"
    );
    assert!(
        asked.status.success(),
        "the command failed: {}",
        String::from_utf8_lossy(&asked.stderr)
    );
    let asked_stdout = String::from_utf8_lossy(&asked.stdout);
    let printed: Vec<&str> = asked_stdout.lines().collect();
    assert!(printed.contains(&"kind: formula"), "{asked_stdout}");
    assert!(printed.contains(&EQUATION_LATEX), "{asked_stdout}");

    let picture = std::fs::canonicalize(support::sample_chapter())
        .unwrap()
        .join("page-num-5/01-figure.png");
    assert!(picture.is_file());
    assert!(
        chart.hits.iter().any(|hit| {
            hit.payload.kind == ItemKind::Figure
                && hit.payload.doc_title == SAMPLE_CHAPTER_TITLE
                && hit.payload.image_path.as_deref() == Some(picture.as_path())
        }),
        "the chart is not among the results:\n{chart}"
    );

    assert!(
        evaluated.status.success(),
        "eval failed: {}",
        String::from_utf8_lossy(&evaluated.stderr)
    );
    let golden = read_golden_questions(&workspace_root().join("golden.toml")).unwrap();
    let evaluated_stdout = String::from_utf8_lossy(&evaluated.stdout);
    let score = evaluated_stdout.lines().last().unwrap_or_default();
    assert!(
        score.starts_with("found in the top 5: ")
            && score.ends_with(&format!(" of {}", golden.len())),
        "the last line is not the score: {score}"
    );
}
