//! The written answer as a document that goes in a file: its blocks in order, its formulas read
//! as formulas, its figures, tables and listings with their captions, and its sources.

use export::{Block, Format, Source};
use gui::contract::{AnswerBlock, ExportPlace, ItemId, ItemKind};
use gui::testkit::{self, sample};
use uuid::Uuid;

const NOTES: &str = "Quanty Sample Notes, chapter 2: Black Scholes In Depth";
const VOLATILITY: &str = "Option Volatility and Pricing, chapter 1: Sample Pages";
const PARAGRAPH: &str = r"The delta \( d_1 \) costs $5 to compute, and `a$b` stays code.";
const TABLE: &str = "| Greek | Measures |\n| --- | --- |\n| Delta | The stock |";
const CODE: &str = "def call_price(spot, strike):\n    return max(spot - strike, 0)";

#[test]
fn an_answer_becomes_a_document_with_its_formulas_figures_tables_code_and_sources() {
    let mut reply = sample::search_reply();
    let mut table = reply.results[0].clone();
    table.number = 4;
    table.id = ItemId(Uuid::from_u128(104));
    table.kind = ItemKind::Table;
    table.label = Some("Table 2.1".to_owned());
    table.caption = Some("Greeks".to_owned());
    table.text = TABLE.to_owned();
    reply.results.push(table);
    let mut listing = reply.results[0].clone();
    listing.number = 5;
    listing.id = ItemId(Uuid::from_u128(105));
    listing.kind = ItemKind::Code;
    listing.label = Some("Listing 2.1".to_owned());
    listing.caption = Some("The call price in Python".to_owned());
    listing.language = Some("python".to_owned());
    listing.last_page = Some(5);
    listing.last_printed_page = Some("9".to_owned());
    listing.text = CODE.to_owned();
    reply.results.push(listing);
    let figure_path = reply.results[2]
        .image
        .clone()
        .expect("the sample figure has a picture")
        .path;
    let mut written = sample::answer();
    written.blocks.push(AnswerBlock::Paragraph {
        text: PARAGRAPH.to_owned(),
        cites: vec![4],
    });
    written.blocks.push(AnswerBlock::Item(4));
    written.blocks.push(AnswerBlock::Item(5));

    let shared = testkit::answered(reply.clone(), sample::concept_graph(), written);
    let document = shared.ask.answer_document().expect("the answer is written");

    assert_eq!(document.title, "Black–Scholes formula and its assumptions");
    assert_eq!(
        document.origin,
        "Question: How is the Black–Scholes formula derived?"
    );
    let text = |markdown: &str, cite: usize| Block::Text {
        markdown: markdown.to_owned(),
        cites: vec![cite],
    };
    assert_eq!(
        document.blocks,
        vec![
            text(
                "The value of the option satisfies a partial differential equation.",
                1
            ),
            Block::Formula {
                latex: reply.results[0].text.clone(),
                label: Some("(2.3)".to_owned()),
                caption: "As printed in the source".to_owned(),
                cite: Some(1),
            },
            Block::Heading("Key assumptions".to_owned()),
            text(
                "The stock price follows a geometric Brownian motion with constant volatility.",
                2
            ),
            text("Three spreads can show the same profit at one price.", 3),
            Block::Figure {
                picture: Some(figure_path.clone()),
                description: reply.results[2].text.clone(),
                caption: "**Figure 13-4**".to_owned(),
                cite: Some(3),
            },
            text(
                r"The delta $d_1$ costs \$5 to compute, and `a$b` stays code.",
                4
            ),
            Block::Table {
                markdown: TABLE.to_owned(),
                caption: "**Table 2.1** Greeks".to_owned(),
                cite: Some(4),
            },
            Block::Code {
                code: CODE.to_owned(),
                language: Some("python".to_owned()),
                caption: "**Listing 2.1** The call price in Python".to_owned(),
                cite: Some(5),
            },
        ]
    );
    let source = |number: usize, text: String| Source { number, text };
    assert_eq!(
        document.sources,
        vec![
            source(1, format!("{NOTES}, p. 7 \u{2014} (2.3)")),
            source(2, format!("{NOTES}, p. 7")),
            source(3, format!("{VOLATILITY}, p. 233 \u{2014} Figure 13-4")),
            source(4, format!("{NOTES}, p. 7 \u{2014} Table 2.1")),
            source(5, format!("{NOTES}, pp. 7\u{2013}9 \u{2014} Listing 2.1")),
        ]
    );

    let request = shared
        .ask
        .export_request(Format::Markdown)
        .expect("the answer can be exported");
    assert_eq!(
        request.file_name,
        "How is the Black–Scholes formula derived.md"
    );
    assert_eq!(
        request.place,
        ExportPlace::Answer {
            generation: shared.ask.generation
        }
    );
    assert_eq!(request.document, document);

    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join(&request.file_name);
    export::write(&request.document, request.format, &path).expect("the file is written");
    let file = std::fs::read_to_string(&path).expect("the file is read");
    assert!(file.contains(r"The delta $d_1$ costs \$5 to compute, and `a$b` stays code. [4]"));
    assert!(
        file.contains("](data:image/png;base64,"),
        "the figure is inside the file"
    );

    let still_searching = testkit::searched(reply);
    assert!(still_searching.ask.answer_document().is_none());
    assert!(still_searching.ask.export_request(Format::Pdf).is_none());
}
