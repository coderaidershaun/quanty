//! The documents the tests write, and the ways to read a written file back.

use std::path::{Path, PathBuf};

use export::{Block, Document, Source};
use lopdf::Object;

/// The picture of the figure in the sample answer: a real page figure from the sample chapters.
pub fn sample_picture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../samples/content/option-volatility-and-pricing/chapter-1/page-num-5/01-figure.png",
    )
}

/// An answer with one block of every kind and two sources, as the desktop app builds it.
pub fn answer_like() -> Document {
    Document {
        title: "Black–Scholes formula and its assumptions".to_owned(),
        origin: "Question: How is the Black–Scholes formula derived?".to_owned(),
        blocks: vec![
            Block::Text {
                markdown: "The price of an option follows from a *partial differential equation* \
                    once the `Delta` hedge removes risk, where the volatility $\\sigma$ is constant."
                    .to_owned(),
                cites: vec![1],
            },
            Block::Heading("Why the hedge removes risk".to_owned()),
            Block::Formula {
                latex: r"\frac{\partial V}{\partial t} + \frac{1}{2}\sigma^2 S^2 \frac{\partial^2 V}{\partial S^2} + rS \frac{\partial V}{\partial S} - rV = 0".to_owned(),
                label: Some("(2.3)".to_owned()),
                caption: "As printed in the source".to_owned(),
                cite: Some(1),
            },
            Block::Figure {
                picture: Some(sample_picture()),
                description: "A chart of volatility against time".to_owned(),
                caption: "**Figure 1.1** Volatility over time".to_owned(),
                cite: Some(2),
            },
            Block::Table {
                markdown: "| Greek | Measures |\n| --- | --- |\n| Delta | Price change per unit of the stock |\n| Vega | Price change per unit of volatility |".to_owned(),
                caption: "**Table 1.1** Greeks".to_owned(),
                cite: Some(1),
            },
            Block::Code {
                code: "def call_price(spot, strike):\n    return max(spot - strike, 0) if spot > 0 else 0\n".to_owned(),
                language: Some("python".to_owned()),
                caption: "**Listing 2.1** The call price in Python".to_owned(),
                cite: Some(2),
            },
        ],
        sources: vec![
            Source {
                number: 1,
                text: "Hull, chapter 2: Options, p. 7".to_owned(),
            },
            Source {
                number: 2,
                text: "Natenberg, chapter 1: Volatility, p. 5 — Figure 1.1".to_owned(),
            },
        ],
    }
}

/// A cheat sheet that no answer is behind: a list, and one block of Markdown with a heading, a
/// paragraph with formulas, a display formula and a table. It cites no source.
pub fn cheat_sheet() -> Document {
    Document {
        title: "Options cheat sheet".to_owned(),
        origin: "Hull, chapter 2: Options".to_owned(),
        blocks: vec![
            Block::List(vec![
                "A call pays $\\max(S - K, 0)$ at the end".to_owned(),
                "A put gives the right to sell at the strike".to_owned(),
                "Delta is the price change per unit of the stock".to_owned(),
            ]),
            Block::Text {
                markdown: "## Put–call parity\n\nFor European options on a stock that pays no dividends, the call $C$ and the put $P$ with the same strike and date are tied together:\n\n$$C - P = S - K e^{-rT}$$\n\n| Symbol | Meaning |\n| --- | --- |\n| $S$ | the stock price now |\n| $K$ | the strike |".to_owned(),
                cites: Vec::new(),
            },
        ],
        sources: Vec::new(),
    }
}

/// All the text of a PDF with every space and line break taken out, because the reader joins two
/// lines with nothing between them.
pub fn pdf_text(path: &Path) -> String {
    let pdf = open_pdf(path);
    let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
    let text = pdf.extract_text(&pages).expect("the pages have text");
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The width and the height in pixels of every picture the PDF holds.
pub fn pdf_pictures(path: &Path) -> Vec<(i64, i64)> {
    let pdf = open_pdf(path);
    pdf.objects
        .values()
        .filter_map(|object| {
            let Object::Stream(stream) = object else {
                return None;
            };
            let is_picture = stream
                .dict
                .get(b"Subtype")
                .and_then(Object::as_name)
                .is_ok_and(|name| name == b"Image");
            let side = |key: &[u8]| stream.dict.get(key).and_then(Object::as_i64).ok();
            if !is_picture {
                return None;
            }
            Some((side(b"Width")?, side(b"Height")?))
        })
        .collect()
}

fn open_pdf(path: &Path) -> lopdf::Document {
    let bytes = std::fs::read(path).expect("the PDF can be read");
    lopdf::Document::load_mem(&bytes).expect("the PDF opens")
}

pub fn text_of(path: &Path) -> String {
    std::fs::read_to_string(path).expect("the file can be read")
}

/// The text between the quotes of every `attribute="…"` in the file, for each place the file
/// has it.
pub fn attribute_values(html: &str, attribute: &str) -> Vec<String> {
    let opener = format!("{attribute}=\"");
    html.match_indices(&opener)
        .map(|(at, _)| {
            let rest = &html[at + opener.len()..];
            rest[..rest.find('"').expect("the attribute is closed")].to_owned()
        })
        .collect()
}
