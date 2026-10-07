//! One small, valid value of every view type, read from the committed sample chapters, so a
//! test of one panel needs no backend.

use std::path::PathBuf;

use uuid::Uuid;

use super::samples_folder;
use crate::contract::{
    Answer, AnswerBlock, Book, Catalogue, ChapterLabel, ChapterState, ConceptGraph, ConceptId,
    DocId, Document, EdgeKind, Failure, FailureKind, GraphEdge, GraphNode, ImageRef, IngestReport,
    ItemCounts, ItemId, ItemKind, NodeId, NodeKind, PageBox, PageConcept, PagePiece, PageToCheck,
    PageView, PieceKind, Preflight, Reason, ResultItem, RetrievalTrace, SearchReply,
};

const NOTES: &str = "quanty-sample-notes";
const VOLATILITY: &str = "option-volatility-and-pricing";

fn id(number: u128) -> Uuid {
    Uuid::from_u128(number)
}

fn image(relative: &str) -> ImageRef {
    ImageRef {
        path: samples_folder().join(relative),
    }
}

fn folder(book: &str, chapter: u32) -> PathBuf {
    samples_folder()
        .join(book)
        .join(format!("chapter-{chapter}"))
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
    let mut formula = result(
        1,
        ItemKind::Formula,
        2,
        "Black Scholes In Depth",
        r"\frac{\partial V}{\partial t} + \tfrac{1}{2}\sigma^2 S^2 \frac{\partial^2 V}{\partial S^2} + rS\frac{\partial V}{\partial S} - rV = 0",
    );
    formula.label = Some("(2.3)".to_owned());
    formula.book = Some("Quanty Sample Notes".to_owned());
    formula.name = Some("Black–Scholes partial differential equation".to_owned());
    let mut assumptions = result(
        2,
        ItemKind::Chunk,
        2,
        "Black Scholes In Depth",
        "The model assumes that the stock price follows a geometric Brownian motion with constant volatility.",
    );
    assumptions.reason = Reason::Concept("Black–Scholes model".to_owned());
    assumptions.page = 1;
    let mut figure = result(
        3,
        ItemKind::Figure,
        3,
        "Option Volatility And Pricing, chapter 1",
        "Three spreads show about the same theoretical profit at an underlying price of 48.40.",
    );
    figure.label = Some("Figure 13-4".to_owned());
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

pub fn catalogue() -> Catalogue {
    let document =
        |number: u128, title: &str, chapter: u32, name: &str, book: &str, pages| Document {
            id: DocId(id(number)),
            title: title.to_owned(),
            chapter: Some(ChapterLabel {
                number: chapter,
                name: name.to_owned(),
            }),
            author: None,
            tags: vec!["options".to_owned()],
            pages: Some(pages),
            items: ItemCounts {
                chunks: 6,
                formulas: 2,
                figures: 1,
                tables: 0,
            },
            ingested_items: Some(9),
            folder: Some(folder(book, chapter)),
        };
    Catalogue {
        books: vec![
            Book {
                title: Some("Quanty Sample Notes".to_owned()),
                chapters: vec![
                    document(1, "Introduction", 1, "Introduction", NOTES, 3),
                    document(
                        2,
                        "Black Scholes In Depth",
                        2,
                        "Black Scholes In Depth",
                        NOTES,
                        3,
                    ),
                ],
            },
            Book {
                title: Some("Option Volatility And Pricing".to_owned()),
                chapters: vec![document(
                    3,
                    "Option Volatility And Pricing, chapter 1",
                    1,
                    "Volatility",
                    VOLATILITY,
                    7,
                )],
            },
        ],
    }
}

pub fn page_view() -> PageView {
    let page = folder(VOLATILITY, 1).join("page-num-5");
    PageView {
        doc: DocId(id(3)),
        page: 5,
        book: Some("Option Volatility And Pricing".to_owned()),
        chapter: Some(ChapterLabel {
            number: 1,
            name: "Volatility".to_owned(),
        }),
        page_count: 7,
        printed_page: Some("233".to_owned()),
        image: Some(ImageRef {
            path: page.join("page.png"),
        }),
        previous_image: Some(ImageRef {
            path: folder(VOLATILITY, 1).join("page-num-4").join("page.png"),
        }),
        next_image: Some(ImageRef {
            path: folder(VOLATILITY, 1).join("page-num-6").join("page.png"),
        }),
        pieces: vec![
            PagePiece {
                number: 1,
                kind: PieceKind::Figure,
                label: Some("Figure 13-4".to_owned()),
                name: None,
                caption: None,
                text: "Three spreads show about the same theoretical profit at an underlying price of 48.40.".to_owned(),
                image: Some(ImageRef {
                    path: page.join("01-figure.png"),
                }),
                cut: Some(PageBox {
                    left: 15,
                    top: 23,
                    right: 905,
                    bottom: 485,
                }),
            },
            PagePiece {
                number: 2,
                kind: PieceKind::Text,
                label: None,
                name: None,
                caption: None,
                text: "At this price the three positions are worth the same.".to_owned(),
                image: None,
                cut: None,
            },
        ],
    }
}

pub fn page_concepts() -> Vec<PageConcept> {
    vec![
        PageConcept {
            id: ConceptId(id(202)),
            name: "Volatility".to_owned(),
            definition: "How far a price moves over a period.".to_owned(),
        },
        PageConcept {
            id: ConceptId(id(203)),
            name: "Straddle".to_owned(),
            definition: "A call and a put with the same strike and date.".to_owned(),
        },
    ]
}

pub fn preflight() -> Preflight {
    Preflight {
        chapter: ChapterLabel {
            number: 1,
            name: "Volatility".to_owned(),
        },
        pages: Some(7),
        state: Some(ChapterState::New),
        blockers: Vec::new(),
    }
}

pub fn ingest_report() -> IngestReport {
    IngestReport {
        doc: DocId(id(3)),
        title: "Option Volatility And Pricing, chapter 1".to_owned(),
        pages: 7,
        items: ItemCounts {
            chunks: 12,
            formulas: 3,
            figures: 2,
            tables: 1,
        },
        concepts_created: 14,
        concepts_linked: 9,
        skipped_items: 0,
        cost_usd: Some(0.42),
        pages_to_check: vec![PageToCheck {
            page: 4,
            reasons: vec!["A figure may be cut short.".to_owned()],
        }],
    }
}

pub fn failure(kind: FailureKind) -> Failure {
    Failure::new(kind, "a failure made for a test")
}
