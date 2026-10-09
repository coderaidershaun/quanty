//! Asks the real `claude` program for one answer from the items of a committed chapter, made into
//! results by hand. Only the real model shows that the schema and the prompt work.

use rag_core::{ClaudeCli, ItemHit, ItemKind, UsageTally};
use rag_retrieval::{ANSWER_MODEL, Reason, SearchHit, SearchResults, answer};

use crate::support::sample_items;

const RUN_COMMAND: &str = "REX_PROD_API=true cargo test -p rag-retrieval --test integration -- --ignored live_answer:: --nocapture";
const QUESTION: &str = "What is the Black–Scholes partial differential equation, and which assumptions does the model rest on?";
const IN_DEPTH_CHAPTER_TITLE: &str = "Quanty Sample Notes, chapter 2: Black Scholes In Depth";
const EQUATION_LATEX: &str = r"\frac{\partial V}{\partial t} + \frac{1}{2}\sigma^2 S^2 \frac{\partial^2 V}{\partial S^2} + r S \frac{\partial V}{\partial S} - r V = 0";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real claude program; run it with: {RUN_COMMAND}"
    );
}

/// `claude` is not started while the key is set.
fn require_no_api_key() {
    assert!(
        std::env::var_os("ANTHROPIC_API_KEY").is_none_or(|key| key.is_empty()),
        "ANTHROPIC_API_KEY is set, so claude would not start; unset it and run again"
    );
}

fn in_depth_results() -> SearchResults {
    let hits = sample_items()
        .into_iter()
        .filter(|item| item.payload.doc_title == IN_DEPTH_CHAPTER_TITLE)
        .map(|item| SearchHit {
            item: ItemHit {
                id: item.id,
                score: 0.5,
                payload: item.payload,
            },
            reason: Reason::Nearest,
        })
        .collect();
    SearchResults {
        hits,
        usage: UsageTally::default(),
    }
}

#[tokio::test]
#[ignore = "calls the real claude program once (one Sonnet question) and spends subscription usage; run with: REX_PROD_API=true cargo test -p rag-retrieval --test integration -- --ignored live_answer:: --nocapture"]
async fn sonnet_cites_by_number_and_returns_a_title_follow_ups_and_symbols_between_the_marks_live()
{
    require_prod_api();
    require_no_api_key();
    let results = in_depth_results();

    let written = answer(&ClaudeCli::new(ANSWER_MODEL), QUESTION, &results)
        .await
        .unwrap_or_else(|error| panic!("no answer: {:#}", anyhow::Error::new(error)));
    println!("--- {QUESTION}\n{written:#?}\n--- printed\n{written}\n");

    let mut failures: Vec<String> = Vec::new();
    let mut check = |holds: bool, what: &str| {
        if !holds {
            failures.push(what.to_owned());
        }
    };
    let sources = || written.claims.iter().flat_map(|claim| &claim.sources);
    check(
        sources().any(|source| {
            source.payload.kind == ItemKind::Formula && source.payload.text == EQUATION_LATEX
        }),
        "no claim names the item of the equation as a source",
    );
    let texts = || written.claims.iter().map(|claim| claim.text.as_str());
    check(
        texts().all(|text| !text.contains("\\frac{\\partial V}{\\partial t}")),
        "a claim retypes the equation",
    );
    check(
        texts().all(|text| {
            !text.contains('$')
                && !text.contains("\\[")
                && text.matches("\\(").count() == text.matches("\\)").count()
                && !text.chars().any(char::is_control)
        }),
        "a claim marks math in another way than between \\( and \\), or holds a control character",
    );
    check(
        written
            .title
            .as_ref()
            .is_some_and(|title| title.split_whitespace().count() <= 12),
        "there is no title, or it is longer than twelve words",
    );
    check(
        (1..=4).contains(&written.follow_ups.len())
            && written
                .follow_ups
                .iter()
                .all(|question| !question.contains('\\')),
        "there is no follow-up question, or one of them holds LaTeX",
    );
    check(
        written
            .claims
            .iter()
            .filter(|claim| claim.heading.is_some())
            .count()
            <= written.claims.len() / 3,
        "more than one claim in three has a heading",
    );
    assert!(
        failures.is_empty(),
        "{} checks failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
