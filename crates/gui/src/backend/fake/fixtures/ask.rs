//! The question, the results, the graph and the answer that every scene asks with.
//!
//! Every text comes from the sample chapters, so the results are what the pages say.

use std::path::Path;

use uuid::Uuid;

use super::library::{self, SampleError, title_of};
use crate::contract::{
    Answer, AnswerBlock, ConceptGraph, DocId, EdgeKind, GraphEdge, GraphNode, ItemId, ItemKind,
    NodeId, NodeKind, Reason, ResultItem, RetrievalTrace, SearchReply,
};

pub(in crate::backend::fake) const QUESTION: &str =
    "How is the Black–Scholes formula derived and what assumptions does it make?";

/// A result of the search: what it is and the piece of a sample page it was stored from.
struct Hit {
    kind: ItemKind,
    doc: u128,
    page: u32,
    piece: u32,
}

const fn hit(kind: ItemKind, doc: u128, page: u32, piece: u32) -> Hit {
    Hit {
        kind,
        doc,
        page,
        piece,
    }
}

/// The results in citation order. No document has more than three of the first eight, as the
/// real search caps them; the ninth is a table that result 4 cites.
const HITS: [Hit; 9] = [
    hit(ItemKind::Formula, 2, 3, 5),
    hit(ItemKind::Formula, 2, 3, 9),
    hit(ItemKind::Chunk, 2, 1, 4),
    hit(ItemKind::Chunk, 1, 2, 4),
    hit(ItemKind::Chunk, 1, 3, 3),
    hit(ItemKind::Chunk, 1, 3, 4),
    hit(ItemKind::Chunk, 3, 1, 2),
    hit(ItemKind::Figure, 3, 5, 1),
    hit(ItemKind::Table, 1, 2, 3),
];

const MODEL: &str = "Black–Scholes model";
const CITED: &str = "Table 1-1";

fn reason_of(number: usize) -> Reason {
    match number {
        5 | 6 => Reason::Concept(MODEL.to_owned()),
        7 => Reason::Concept("Volatility".to_owned()),
        9 => Reason::Cited {
            by: 4,
            label: CITED.to_owned(),
        },
        _ => Reason::Nearest,
    }
}

fn item(samples: &Path, number: usize, hit: &Hit) -> Result<ResultItem, SampleError> {
    let doc = DocId(Uuid::from_u128(hit.doc));
    let page = library::page(samples, doc, hit.page)?;
    let piece = page
        .pieces
        .iter()
        .find(|piece| piece.number == hit.piece)
        .ok_or(SampleError::NoSuchPiece {
            doc: doc.0,
            page: hit.page,
            piece: hit.piece,
        })?;
    let chapter = page.chapter.clone().unwrap_or_default();
    let book = page.book.clone().unwrap_or_default();
    let is_chunk = hit.kind == ItemKind::Chunk;
    Ok(ResultItem {
        number,
        id: ItemId(Uuid::from_u128(0x1000 + number as u128)),
        kind: hit.kind,
        score: 0.93 - 0.04 * number as f32,
        reason: reason_of(number),
        doc,
        doc_title: title_of(&book, chapter.number, &chapter.name),
        book: page.book.clone(),
        page: hit.page,
        printed_page: page.printed_page.clone(),
        label: piece.label.clone().filter(|_| !is_chunk),
        text: piece.text.clone(),
        image: piece.image.clone().filter(|_| hit.kind == ItemKind::Figure),
        piece: (!is_chunk).then_some(piece.number),
        caption: piece.caption.clone().filter(|_| !is_chunk),
        name: piece.name.clone().filter(|_| !is_chunk),
    })
}

fn names(names: &[&str]) -> Option<Vec<String>> {
    Some(names.iter().map(|name| (*name).to_owned()).collect())
}

/// What the search finds for the question: nine results and the trace of how it found them.
///
/// # Errors
/// When a sample page cannot be read.
pub(in crate::backend::fake) fn reply(samples: &Path) -> Result<SearchReply, SampleError> {
    let results = HITS
        .iter()
        .enumerate()
        .map(|(at, hit)| item(samples, at + 1, hit))
        .collect::<Result<Vec<_>, _>>()?;
    let passed_over = results
        .iter()
        .find(|result| result.doc == DocId(Uuid::from_u128(2)))
        .map(|result| vec![(result.doc_title.clone(), 2)])
        .unwrap_or_default();
    Ok(SearchReply {
        results,
        trace: RetrievalTrace {
            documents_searched: None,
            nearest: 8,
            seed_concepts: names(&[MODEL, "Volatility", "Geometric Brownian motion"]),
            related_concepts: names(&["Delta hedging", "Option pricing", "Put–call parity"]),
            candidates: Some(14),
            ranked: Some(14),
            kept: Some(8),
            passed_over,
            cited: names(&[CITED]),
        },
    })
}

/// The same results, found by a search that met no concept in the graph.
///
/// # Errors
/// When a sample page cannot be read.
pub(in crate::backend::fake) fn reply_without_concepts(
    samples: &Path,
) -> Result<SearchReply, SampleError> {
    let mut found = reply(samples)?;
    found.trace.seed_concepts = Some(Vec::new());
    found.trace.related_concepts = Some(Vec::new());
    found.trace.cited = Some(Vec::new());
    Ok(found)
}

/// A search that found nothing, after every step of the search ran.
pub(in crate::backend::fake) fn no_result() -> SearchReply {
    SearchReply {
        results: Vec::new(),
        trace: RetrievalTrace {
            nearest: 0,
            seed_concepts: Some(Vec::new()),
            related_concepts: Some(Vec::new()),
            candidates: Some(0),
            ranked: Some(0),
            kept: Some(0),
            cited: Some(Vec::new()),
            ..RetrievalTrace::default()
        },
    }
}

/// A search over a filter that no document carries: it stops before it embeds the question.
pub(in crate::backend::fake) fn no_document_has_the_labels() -> SearchReply {
    SearchReply {
        results: Vec::new(),
        trace: RetrievalTrace {
            documents_searched: Some(0),
            nearest: 0,
            ..RetrievalTrace::default()
        },
    }
}

fn concept_node(number: u128, kind: NodeKind) -> GraphNode {
    let concept = library::concept(number);
    GraphNode {
        id: NodeId::Concept(concept.id),
        kind,
        label: concept.name,
        detail: Some(concept.definition),
    }
}

fn result_node(number: usize, kind: NodeKind, label: &str, detail: Option<&str>) -> GraphNode {
    GraphNode {
        id: NodeId::Item(number),
        kind,
        label: label.to_owned(),
        detail: detail.map(str::to_owned),
    }
}

fn concept_at(number: u128) -> NodeId {
    NodeId::Concept(library::concept_id(number))
}

fn edge(from: NodeId, to: NodeId, kind: EdgeKind) -> GraphEdge {
    GraphEdge { from, to, kind }
}

/// Four concepts, two of them one relation away from the search, and the three results that
/// mention them, with one edge of each kind.
pub(in crate::backend::fake) fn graph() -> ConceptGraph {
    ConceptGraph {
        nodes: vec![
            concept_node(1, NodeKind::Concept),
            concept_node(2, NodeKind::Concept),
            concept_node(3, NodeKind::Related),
            concept_node(4, NodeKind::Related),
            result_node(
                1,
                NodeKind::Formula,
                "Formula (2.4)",
                Some("Black–Scholes call price"),
            ),
            result_node(
                2,
                NodeKind::Formula,
                "Formula (2.6)",
                Some("The terms d one and d two"),
            ),
            result_node(8, NodeKind::Figure, "Figure 13-4", None),
        ],
        edges: vec![
            edge(concept_at(1), concept_at(2), EdgeKind::Assumes),
            edge(concept_at(1), concept_at(3), EdgeKind::DerivedFrom),
            edge(concept_at(4), concept_at(1), EdgeKind::Generalises),
            edge(concept_at(2), concept_at(4), EdgeKind::PartOf),
            edge(concept_at(3), concept_at(4), EdgeKind::UsedFor),
            edge(NodeId::Item(1), concept_at(1), EdgeKind::Mentions),
            edge(NodeId::Item(2), concept_at(1), EdgeKind::Mentions),
            edge(NodeId::Item(8), concept_at(2), EdgeKind::Mentions),
        ],
    }
}

fn paragraph(text: &str, cites: &[usize]) -> AnswerBlock {
    AnswerBlock::Paragraph {
        text: text.to_owned(),
        cites: cites.to_vec(),
    }
}

fn follow_ups() -> Vec<String> {
    [
        "How is the Black–Scholes partial differential equation derived?",
        "What does put–call parity say, and why does it hold in any model?",
        "Which assumptions of the model do real markets break?",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The answer to the question: the formula, the hedging argument, each assumption, and the
/// table and the figure that the results add.
pub(in crate::backend::fake) fn answer() -> Answer {
    let mut blocks = vec![
        paragraph(
            r"For a European call the model gives the price \( C = S\,N(d_1) - K e^{-rT} N(d_2) \), where \( N \) is the cumulative distribution function of the standard normal distribution.",
            &[1],
        ),
        AnswerBlock::Item(1),
        paragraph(
            r"The two numbers \( d_1 \) and \( d_2 \) are the same for the call and the put, so one calculation of them prices both.",
            &[2],
        ),
        AnswerBlock::Item(2),
        paragraph(
            "The derivation rests on a hedging argument. Hold one option and sell short the right amount of the stock, and for a very short moment the position has no risk, so it must earn exactly the risk-free rate. Setting its return equal to that rate is what fixes the price.",
            &[3, 5],
        ),
        AnswerBlock::Heading("Key assumptions".to_owned()),
    ];
    for assumption in [
        "The stock pays no dividends during the life of the option.",
        "The volatility of the stock and the risk-free interest rate are constants.",
        "Trading is continuous and costs nothing, and any amount of stock can be bought or sold short.",
        "There is no way to make a risk-free profit.",
        "The option is European, so it can be exercised only at expiration.",
    ] {
        blocks.push(paragraph(assumption, &[3]));
    }
    blocks.push(paragraph(
        "How the price of a call and of a put respond when one input rises is set out in the table.",
        &[4, 9],
    ));
    blocks.push(AnswerBlock::Item(9));
    blocks.push(paragraph(
        "Figure 13-4 shows that three spreads can earn about the same theoretical profit at one price of the stock and still differ in risk for large moves.",
        &[8],
    ));
    blocks.push(AnswerBlock::Item(8));
    Answer {
        title: Some("Black–Scholes formula, derivation and assumptions".to_owned()),
        blocks,
        follow_ups: follow_ups(),
    }
}

/// An answer that says the results do not answer the question, and still offers questions.
pub(in crate::backend::fake) fn answer_without_blocks() -> Answer {
    Answer {
        title: None,
        blocks: Vec::new(),
        follow_ups: follow_ups(),
    }
}
