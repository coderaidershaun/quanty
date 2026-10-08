//! One small, valid search reply, answer and concept graph, written by hand, so a test of one
//! panel needs no backend. Only the picture of the figure is a file of the committed samples.

use uuid::Uuid;

use super::samples_folder;
use crate::contract::{
    Answer, AnswerBlock, ConceptGraph, ConceptId, DocId, EdgeKind, GraphEdge, GraphNode, ImageRef,
    ItemId, ItemKind, NodeId, NodeKind, Reason, ResultItem, RetrievalTrace, SearchReply,
};

const NOTES_TITLE: &str = "Quanty Sample Notes";
const NOTES_CHAPTER_2: &str = "Black Scholes In Depth";
const VOLATILITY: &str = "option-volatility-and-pricing";
const VOLATILITY_TITLE: &str = "Option Volatility and Pricing";
const VOLATILITY_CHAPTER: &str = "Sample Pages";

fn id(number: u128) -> Uuid {
    Uuid::from_u128(number)
}

fn image(relative: &str) -> ImageRef {
    ImageRef {
        path: samples_folder().join(relative),
    }
}

/// The title an ingest gives a chapter.
fn chapter_title(book: &str, chapter: u32, name: &str) -> String {
    format!("{book}, chapter {chapter}: {name}")
}

fn result(number: usize, kind: ItemKind, doc: u128, title: &str, text: &str) -> ResultItem {
    ResultItem {
        number,
        id: ItemId(id(100 + number as u128)),
        kind,
        score: 0.9 - number as f32 / 10.0,
        reason: Reason::Nearest,
        doc: DocId(id(doc)),
        doc_title: title.to_owned(),
        book: None,
        page: 3,
        printed_page: Some("7".to_owned()),
        label: None,
        text: text.to_owned(),
        image: None,
        piece: Some(1),
        caption: None,
        name: None,
    }
}

pub fn search_reply() -> SearchReply {
    let black_scholes = chapter_title(NOTES_TITLE, 2, NOTES_CHAPTER_2);
    let mut formula = result(
        1,
        ItemKind::Formula,
        2,
        &black_scholes,
        r"\frac{\partial V}{\partial t} + \tfrac{1}{2}\sigma^2 S^2 \frac{\partial^2 V}{\partial S^2} + rS\frac{\partial V}{\partial S} - rV = 0",
    );
    formula.label = Some("(2.3)".to_owned());
    formula.book = Some(NOTES_TITLE.to_owned());
    formula.name = Some("Black–Scholes partial differential equation".to_owned());
    let mut assumptions = result(
        2,
        ItemKind::Chunk,
        2,
        &black_scholes,
        "The model assumes that the stock price follows a geometric Brownian motion with constant volatility.",
    );
    assumptions.reason = Reason::Concept("Black–Scholes model".to_owned());
    assumptions.book = Some(NOTES_TITLE.to_owned());
    assumptions.page = 1;
    // A passage can hold several pieces of a page, so it names none.
    assumptions.piece = None;
    let mut figure = result(
        3,
        ItemKind::Figure,
        3,
        &chapter_title(VOLATILITY_TITLE, 1, VOLATILITY_CHAPTER),
        "Three spreads show about the same theoretical profit at an underlying price of 48.40.",
    );
    figure.label = Some("Figure 13-4".to_owned());
    figure.book = Some(VOLATILITY_TITLE.to_owned());
    figure.page = 5;
    figure.printed_page = Some("233".to_owned());
    figure.image = Some(image(&format!(
        "{VOLATILITY}/chapter-1/page-num-5/01-figure.png"
    )));
    SearchReply {
        results: vec![formula, assumptions, figure],
        trace: RetrievalTrace {
            documents_searched: None,
            nearest: 3,
            seed_concepts: Some(vec!["Black–Scholes model".to_owned()]),
            related_concepts: Some(vec!["Volatility".to_owned()]),
            candidates: Some(4),
            ranked: Some(4),
            kept: Some(3),
            passed_over: Vec::new(),
            cited: Some(Vec::new()),
        },
    }
}

pub fn answer() -> Answer {
    Answer {
        title: Some("Black–Scholes formula and its assumptions".to_owned()),
        blocks: vec![
            AnswerBlock::Paragraph {
                text: "The value of the option satisfies a partial differential equation."
                    .to_owned(),
                cites: vec![1],
            },
            AnswerBlock::Item(1),
            AnswerBlock::Heading("Key assumptions".to_owned()),
            AnswerBlock::Paragraph {
                text:
                    "The stock price follows a geometric Brownian motion with constant volatility."
                        .to_owned(),
                cites: vec![2],
            },
            AnswerBlock::Paragraph {
                text: "Three spreads can show the same profit at one price.".to_owned(),
                cites: vec![3],
            },
            AnswerBlock::Item(3),
        ],
        follow_ups: vec![
            "What does the volatility term do in the equation?".to_owned(),
            "How is the equation solved for a call option?".to_owned(),
        ],
    }
}

pub fn concept_graph() -> ConceptGraph {
    let model = NodeId::Concept(ConceptId(id(201)));
    let volatility = NodeId::Concept(ConceptId(id(202)));
    let node = |id, kind, label: &str| GraphNode {
        id,
        kind,
        label: label.to_owned(),
        detail: None,
    };
    ConceptGraph {
        nodes: vec![
            node(model, NodeKind::Concept, "Black–Scholes model"),
            node(volatility, NodeKind::Related, "Volatility"),
            node(NodeId::Item(1), NodeKind::Formula, "Formula (2.3)"),
        ],
        edges: vec![
            GraphEdge {
                from: model,
                to: volatility,
                kind: EdgeKind::Assumes,
            },
            GraphEdge {
                from: NodeId::Item(1),
                to: model,
                kind: EdgeKind::Mentions,
            },
        ],
    }
}
