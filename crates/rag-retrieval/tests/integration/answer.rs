//! Writes answers from items made by hand, with a language model that follows a rule, so that
//! what the model is given and what is printed are seen without a model call.

use std::path::PathBuf;

use rag_core::{DocId, ItemHit, ItemId, ItemKind, ItemPayload};
use rag_ingestion::testing::StandInLlm;
use rag_retrieval::{AnswerError, Reason, SearchHit, SearchResults, answer};
use serde_json::{Value, json};

const QUESTION: &str = "What is the Black–Scholes equation, and which chart shows the payoff?";
const NOTES_TITLE: &str = "Quanty Sample Notes, chapter 2: Black Scholes In Depth";
const BOOK_TITLE: &str = "Option Volatility and Pricing, chapter 1: Sample Pages";
const CHUNK_TEXT: &str =
    "A call is worth its payoff at expiration.\n\nBefore then, time adds value.";
const LATEX: &str =
    "\\frac{\\partial V}{\\partial t} + r S \\frac{\\partial V}{\\partial S}\n - r V = 0";
const FIGURE_TEXT: &str =
    "A chart of the profit of a long straddle against the price of the stock.";
const PICTURE: &str = "/content/option-volatility/page-num-5/01-figure.png";

fn hit(kind: ItemKind, position: u32, payload: ItemPayload, reason: Reason) -> SearchHit {
    SearchHit {
        item: ItemHit {
            id: ItemId::new(payload.doc_id, kind, position),
            score: 0.5,
            payload,
        },
        reason,
    }
}

fn payload(document: &str, title: &str, kind: ItemKind, page: &str, text: &str) -> ItemPayload {
    ItemPayload {
        doc_id: DocId::from_source_sha256(document),
        doc_title: title.to_owned(),
        page: 1,
        printed_page: Some(page.to_owned()),
        kind,
        text: text.to_owned(),
        image_path: None,
        label: None,
        cites: Vec::new(),
    }
}

/// A chunk, a formula that the chunk cites, and a figure of another document that came in through
/// the graph.
fn found_items() -> SearchResults {
    let chunk = payload("notes", NOTES_TITLE, ItemKind::Chunk, "5", CHUNK_TEXT);
    let formula = ItemPayload {
        label: Some("(7.3)".to_owned()),
        ..payload("notes", NOTES_TITLE, ItemKind::Formula, "7", LATEX)
    };
    let figure = ItemPayload {
        label: Some("Figure 13-4".to_owned()),
        image_path: Some(PathBuf::from(PICTURE)),
        ..payload("book", BOOK_TITLE, ItemKind::Figure, "233", FIGURE_TEXT)
    };
    SearchResults {
        hits: vec![
            hit(ItemKind::Chunk, 0, chunk, Reason::Nearest),
            hit(
                ItemKind::Formula,
                1,
                formula,
                Reason::Cited {
                    by: 1,
                    label: "(7.3)".to_owned(),
                },
            ),
            hit(
                ItemKind::Figure,
                2,
                figure,
                Reason::Concept("straddle".to_owned()),
            ),
        ],
    }
}

fn replying(claims: Value) -> StandInLlm {
    StandInLlm::replying(move |_, _| Ok(json!({ "claims": claims })))
}

#[tokio::test]
async fn an_answer_cites_document_and_printed_page_and_keeps_the_latex_of_a_formula_unchanged() {
    let llm = replying(json!([
        { "text": "The Black–Scholes equation relates the price to time and to the stock.", "sources": [1, 2] },
        { "text": "The payoff of a straddle is drawn in a chart.", "sources": [3] },
    ]));

    let answer_found = answer(&llm, QUESTION, &found_items()).await.unwrap();

    assert_eq!(llm.calls(), 1, "one question, one call");
    let asked = &llm.questions()[0];
    assert!(!asked.system_prompt.is_empty());
    assert!(!asked.schema.is_empty());
    assert!(
        asked
            .input
            .starts_with(&format!("Question: {QUESTION}\n\nItem 1\n")),
        "{}",
        asked.input
    );
    let chunk_block =
        format!("Item 1\ndocument: {NOTES_TITLE}\npage: 5\nkind: chunk\ntext:\n{CHUNK_TEXT}");
    let formula_block = format!(
        "Item 2\ndocument: {NOTES_TITLE}\npage: 7\nkind: formula\nlabel: (7.3)\nlatex:\n{LATEX}"
    );
    let figure_block = format!(
        "Item 3\ndocument: {BOOK_TITLE}\npage: 233\nkind: figure\nlabel: Figure 13-4\npicture: {PICTURE}\nexplanation:\n{FIGURE_TEXT}"
    );
    for block in [chunk_block, formula_block, figure_block] {
        assert!(
            asked.input.contains(&block),
            "{block}\n---\n{}",
            asked.input
        );
    }

    let printed = answer_found.to_string();
    assert!(
        printed.starts_with(
            "The Black–Scholes equation relates the price to time and to the stock.\n"
        ),
        "{printed}"
    );
    let cited_lines = [
        format!("  source: {NOTES_TITLE}, page 5 (chunk)"),
        format!("  source: {NOTES_TITLE}, page 7 (formula (7.3))"),
        format!("  source: {BOOK_TITLE}, page 233 (figure Figure 13-4)"),
        format!("  picture: {PICTURE}"),
        "The payoff of a straddle is drawn in a chart.".to_owned(),
    ];
    for line in &cited_lines {
        assert!(
            printed.lines().any(|printed_line| printed_line == line),
            "{line}\n---\n{printed}"
        );
    }
    assert!(
        printed.contains(&format!(
            "  source: {NOTES_TITLE}, page 7 (formula (7.3))\n{LATEX}\n"
        )),
        "the formula is printed as the document has it, on lines of its own, under its source line:\n{printed}"
    );

    // A reply that breaks a rule is an error that says which rule.
    let unknown = replying(json!([{ "text": "A claim.", "sources": [1, 99] }]));
    let error = answer(&unknown, QUESTION, &found_items())
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            AnswerError::UnknownSource {
                claim: 1,
                number: 99,
                items: 3
            }
        ),
        "{error:?}"
    );

    let bare = replying(json!([
        { "text": "A claim with a source.", "sources": [1] },
        { "text": "A claim with none.", "sources": [] },
    ]));
    let error = answer(&bare, QUESTION, &found_items()).await.unwrap_err();
    assert!(
        matches!(error, AnswerError::NoSource { claim: 2 }),
        "{error:?}"
    );
}
