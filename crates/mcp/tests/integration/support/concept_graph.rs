//! A throwaway library whose concepts, mentions and relations are scripted, so that a test knows
//! every node and edge of its graph. The model finds a concept in an item by words that the item
//! really holds.

use std::path::Path;

use rag_core::concept_input;
use rag_ingestion::testing::vector_at;
use rag_ingestion::{Item, chapter_items};
use serde_json::{Value, json};

use super::Library;
use super::{sample_chapter_folder, sample_content_folder};

pub const LONG_STRADDLE: &str = "Long straddle";
pub const TIME_VALUE: &str = "Time value";
pub const BLACK_SCHOLES: &str = "Black–Scholes model";
pub const VOLATILITY: &str = "Volatility";
pub const ITO: &str = "Itô's lemma";
pub const STOCHASTIC_CALCULUS: &str = "Stochastic calculus";

/// A second name for the Black–Scholes model, which an item of the second notes chapter gives and
/// the ingest links to the first name because their vectors are the same.
pub const ALIAS: &str = "Black–Scholes–Merton model";

pub const BLACK_SCHOLES_DEFINITION: &str = "A formula for the price of an option on a stock.";
pub const VOLATILITY_DEFINITION: &str = "How much a price moves over time.";

/// The question that finds the concept Volatility when a test places it at that concept's vector.
pub const VOLATILITY_QUERY: &str = "how much does a price move";

const ALIAS_DEFINITION: &str = "The pricing formula with the Merton extension.";

/// What the scripted model finds in an item: the concepts and relations of every rule whose words
/// the item holds.
struct Rule {
    words: &'static str,
    concepts: &'static [(&'static str, &'static str)],
    relations: &'static [(&'static str, &'static str, &'static str)],
}

const RULES: [Rule; 9] = [
    Rule {
        words: "straddle",
        concepts: &[(
            LONG_STRADDLE,
            "A bet that the price moves a lot, either way.",
        )],
        relations: &[],
    },
    Rule {
        words: "intrinsic value",
        concepts: &[(
            TIME_VALUE,
            "What an option is worth above its payoff today.",
        )],
        relations: &[],
    },
    Rule {
        words: "described in the section that follows",
        concepts: &[(BLACK_SCHOLES, BLACK_SCHOLES_DEFINITION)],
        relations: &[],
    },
    Rule {
        words: "rests on a simple observation",
        concepts: &[(BLACK_SCHOLES, BLACK_SCHOLES_DEFINITION)],
        relations: &[],
    },
    Rule {
        words: "Volatility* is the one input",
        concepts: &[
            (VOLATILITY, VOLATILITY_DEFINITION),
            (BLACK_SCHOLES, BLACK_SCHOLES_DEFINITION),
        ],
        relations: &[(BLACK_SCHOLES, "ASSUMES", VOLATILITY)],
    },
    Rule {
        words: "This chapter derives it",
        concepts: &[(ALIAS, ALIAS_DEFINITION)],
        relations: &[],
    },
    Rule {
        words: "scales that randomness",
        concepts: &[(VOLATILITY, VOLATILITY_DEFINITION)],
        relations: &[],
    },
    Rule {
        words: "known as *Itô's lemma*",
        concepts: &[
            (
                ITO,
                "The rule for the change of a function of a random path.",
            ),
            (STOCHASTIC_CALCULUS, "The calculus of random paths."),
        ],
        relations: &[
            (BLACK_SCHOLES, "DERIVED_FROM", ITO),
            (STOCHASTIC_CALCULUS, "GENERALISES", ITO),
        ],
    },
    Rule {
        words: "Black–Scholes partial differential equation",
        concepts: &[(BLACK_SCHOLES, BLACK_SCHOLES_DEFINITION)],
        relations: &[],
    },
];

/// Only the item's own text counts: the question of an item that stands alone also carries the
/// text of the stored items nearest to it.
fn scripted_reply(question: &str) -> Value {
    let own_text = question
        .split("\n\nPossibly related material")
        .next()
        .unwrap_or(question);
    let matching: Vec<&Rule> = RULES
        .iter()
        .filter(|rule| own_text.contains(rule.words))
        .collect();
    let concepts: Vec<Value> = matching
        .iter()
        .flat_map(|rule| rule.concepts)
        .map(|(name, definition)| json!({ "name": name, "definition": definition }))
        .collect();
    let relations: Vec<Value> = matching
        .iter()
        .flat_map(|rule| rule.relations)
        .map(|(from, kind, to)| json!({ "from": from, "type": kind, "to": to }))
        .collect();
    json!({ "concepts": concepts, "relations": relations })
}

/// The item of the chapter in `folder` whose text holds `words`.
pub fn item_saying(folder: &Path, words: &str) -> Item {
    let chapter = ocr::read_chapter(folder).expect("the sample chapter should read");
    chapter_items(&chapter)
        .into_iter()
        .find(|item| item.input.text.contains(words))
        .unwrap_or_else(|| panic!("no item of {} holds {words:?}", folder.display()))
}

/// The words of the item of the first notes chapter that `similar_items` is shown to find next to
/// the figure of page 5 of the sample chapter.
pub const NEAR_THE_FIGURE: &str = "is a contract that gives its holder a right";

/// How near to the figure the item above is placed.
pub const NEAR_THE_FIGURE_SCORE: f32 = 0.9;

impl Library {
    /// The three sample chapters ingested with a model that finds six concepts. Black–Scholes
    /// model is in two items of each notes chapter, under two names, and has the relations
    /// `ASSUMES` Volatility and `DERIVED_FROM` Itô's lemma; Stochastic calculus `GENERALISES`
    /// Itô's lemma; Long straddle and Time value have no relation.
    pub async fn ingested_with_concept_graph(test_name: &str) -> Library {
        let same_concept = vector_at(0.5, 11);
        let near_the_figure = item_saying(
            &sample_content_folder().join("quanty-sample-notes/chapter-1"),
            NEAR_THE_FIGURE,
        )
        .input
        .text;
        let placements = vec![
            (
                concept_input(BLACK_SCHOLES, BLACK_SCHOLES_DEFINITION).text,
                same_concept.clone(),
            ),
            (concept_input(ALIAS, ALIAS_DEFINITION).text, same_concept),
            (
                concept_input(VOLATILITY, VOLATILITY_DEFINITION).text,
                volatility_vector(),
            ),
            (near_the_figure, vector_at(NEAR_THE_FIGURE_SCORE, 13)),
        ];
        Library::ingested_finding(test_name, scripted_reply, placements).await
    }
}

/// Where the concept Volatility is placed, and so where a test places `VOLATILITY_QUERY`.
pub fn volatility_vector() -> Vec<f32> {
    vector_at(0.5, 12)
}

/// The item of the sample chapter's page 5 that the figure search starts from.
pub fn figure_of_page_5() -> Item {
    chapter_items(
        &ocr::read_chapter(&sample_chapter_folder()).expect("the sample chapter should read"),
    )
    .into_iter()
    .find(|item| item.payload.kind == rag_core::ItemKind::Figure && item.payload.page == 5)
    .expect("page 5 of the sample chapter has a figure")
}
