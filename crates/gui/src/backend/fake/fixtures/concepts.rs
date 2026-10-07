//! The concepts the scenes answer with. A page and the graph of an ask both take a concept from
//! here, so it has the same id, name and definition in both.

use uuid::Uuid;

use crate::contract::{ConceptId, DocId, PageConcept};

/// The concepts that sample pages mention: the number that names the concept, its name and its
/// definition. The graph of an ask uses the same numbers.
const CONCEPTS: [(u128, &str, &str); 9] = [
    (
        1,
        "Black–Scholes model",
        "A model that prices a European option on a stock from five inputs, by hedging the option away.",
    ),
    (
        2,
        "Volatility",
        "How much the price of the stock moves, as a yearly standard deviation of its returns.",
    ),
    (
        3,
        "Delta hedging",
        "Holding the amount of stock that cancels the first-order moves of an option.",
    ),
    (
        4,
        "Option pricing",
        "Finding the fair price to pay today for the right to trade later at a fixed price.",
    ),
    (
        5,
        "Geometric Brownian motion",
        "A random path in which the stock moves by a drift plus a shock that grows with its price.",
    ),
    (
        6,
        "Put–call parity",
        "The link between the price of a call and of a put with the same strike and expiration.",
    ),
    (
        7,
        "Option payoff",
        "What the holder of an option receives on the expiration date.",
    ),
    (
        8,
        "Forward price",
        "The price agreed today for a trade that happens later.",
    ),
    (
        9,
        "Volatility surface",
        "Implied volatility drawn against exercise price and months to expiration.",
    ),
];

/// The concepts of each page that has any: document, page and the numbers in `CONCEPTS`. Every
/// page that is not listed has none.
const PAGE_CONCEPTS: [(u128, u32, &[u128]); 10] = [
    (1, 1, &[7]),
    (1, 3, &[1, 3]),
    (2, 1, &[1, 5]),
    (2, 2, &[3, 1]),
    (2, 3, &[1, 6, 2]),
    (3, 1, &[8]),
    (3, 2, &[8]),
    (3, 4, &[2]),
    (3, 5, &[2]),
    (3, 7, &[9, 2]),
];

/// The concept the number names in `CONCEPTS`.
///
/// # Panics
/// When the number is not in `CONCEPTS`: it is a mistake in the fixtures, never a runtime input.
pub(super) fn concept(number: u128) -> PageConcept {
    let (_, name, definition) = CONCEPTS
        .iter()
        .find(|(known, ..)| *known == number)
        .unwrap_or_else(|| panic!("the fixtures name no concept {number}"));
    PageConcept {
        id: concept_id(number),
        name: (*name).to_owned(),
        definition: (*definition).to_owned(),
    }
}

pub(super) fn concept_id(number: u128) -> ConceptId {
    ConceptId(Uuid::from_u128(0xC0_0000 + number))
}

/// The concepts that the sample page mentions, most mentioned first. Many pages have none.
pub(in crate::backend::fake) fn page_concepts(doc: DocId, page: u32) -> Vec<PageConcept> {
    PAGE_CONCEPTS
        .iter()
        .find(|(number, listed, _)| doc.0 == Uuid::from_u128(*number) && *listed == page)
        .map(|(_, _, numbers)| numbers.iter().map(|number| concept(*number)).collect())
        .unwrap_or_default()
}
