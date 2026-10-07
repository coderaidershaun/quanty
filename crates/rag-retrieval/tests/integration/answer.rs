//! Writes answers from items made by hand, with a language model that follows a rule, so that
//! what the model is given and what is printed are seen without a model call.

use std::path::PathBuf;

use rag_core::{DocId, DocumentLabels, ItemHit, ItemId, ItemKind, ItemPayload};
use rag_ingestion::testing::StandInLlm;
use rag_retrieval::{AnswerError, Reason, SearchHit, SearchResults, Source, answer};
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
        document_labels: DocumentLabels::default(),
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

fn assert_send<T: Send>(value: T) -> T {
    value
}

#[tokio::test]
async fn an_answer_gives_the_number_of_each_source_a_title_headings_and_follow_up_questions() {
    let llm = StandInLlm::replying(|_, _| {
        Ok(json!({
            "claims": [
                { "heading": "", "text": "The equation holds for every option.", "sources": [2, 1, 2] },
                {
                    "heading": "  Key\nassumptions ",
                    // Do not write `\\theta` below. The `\t` is a real tab, which is what JSON
                    // gives when the model writes the command with one backslash.
                    "text": "The volatility \\( \\sigma \\) and the angle \\( \theta \\) are constant.",
                    "sources": [3],
                },
            ],
            "title": " The Black–Scholes equation\nand its chart ",
            "follow_ups": [
                " How is the Black–Scholes equation solved for a call option? ",
                "",
                "What does a long straddle pay at expiration?",
                "How is the Black–Scholes equation solved for a call option?",
                "Why does the growth rate of the stock drop out of the Black–Scholes equation?",
                "How does volatility change the price of a straddle?",
                "What is a butterfly spread?",
            ],
        }))
    });
    let results = found_items();

    let written = assert_send(answer(&llm, QUESTION, &results)).await.unwrap();

    let schema: Value = serde_json::from_str(&llm.questions()[0].schema).unwrap();
    assert_eq!(schema["required"], json!(["claims", "title", "follow_ups"]));
    assert_eq!(
        schema["properties"]["claims"]["items"]["required"],
        json!(["heading", "text", "sources"]),
        "a field that the schema does not ask for is never sent, and the reader takes that as blank"
    );

    assert_eq!(
        written.title.as_deref(),
        Some("The Black–Scholes equation and its chart")
    );
    let numbers = |claim: usize| -> Vec<usize> {
        written.claims[claim]
            .sources
            .iter()
            .map(|source| source.number)
            .collect()
    };
    assert_eq!(numbers(0), [2, 1], "in the order named, each once");
    assert_eq!(numbers(1), [3]);
    for Source { number, payload } in written.claims.iter().flat_map(|claim| &claim.sources) {
        assert_eq!(payload, &results.hits[number - 1].item.payload);
    }
    assert_eq!(written.claims[0].heading, None);
    assert_eq!(
        written.claims[1].heading.as_deref(),
        Some("Key assumptions")
    );
    assert_eq!(
        written.claims[1].text,
        "The volatility \\( \\sigma \\) and the angle \\( \\theta \\) are constant.",
        "a tab is the backslash and the t of a LaTeX command that JSON read as one character"
    );
    assert_eq!(
        written.follow_ups,
        [
            "How is the Black–Scholes equation solved for a call option?",
            "What does a long straddle pay at expiration?",
            "Why does the growth rate of the stock drop out of the Black–Scholes equation?",
            "How does volatility change the price of a straddle?",
        ],
        "blank and repeated questions are left out, and four are kept"
    );
    let printed = written.to_string();
    for unprinted in ["its chart", "Key assumptions", "long straddle pay"] {
        assert!(!printed.contains(unprinted), "{printed}");
    }
    assert!(
        printed.starts_with(&format!(
            "The equation holds for every option.\n  source: {NOTES_TITLE}, page 7 (formula (7.3))\n{LATEX}\n  source: {NOTES_TITLE}, page 5 (chunk)\n\n"
        )),
        "{printed}"
    );

    // A reply that holds only claims is an answer with nothing else, and no claim means no title.
    let bare = replying(json!([{ "text": "A claim.", "sources": [1] }]));
    let written = answer(&bare, QUESTION, &results).await.unwrap();
    assert_eq!(written.title, None);
    assert_eq!(written.claims[0].heading, None);
    assert!(written.follow_ups.is_empty());
    let unanswered = StandInLlm::replying(|_, _| {
        Ok(json!({ "claims": [], "title": "A title", "follow_ups": ["What is a put option?"] }))
    });
    let written = answer(&unanswered, QUESTION, &results).await.unwrap();
    assert_eq!(written.title, None);
    assert_eq!(written.follow_ups, ["What is a put option?"]);
}
