//! A document written as an HTML file, a Markdown file and a PDF.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use export::{Format, write};

use crate::support;

#[test]
fn an_answer_is_written_whole_and_standalone_in_every_format() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for (format, file, check) in [
        (
            Format::Html,
            "answer.html",
            html_holds_the_whole_document_typeset_and_standalone as fn(&Path),
        ),
        (
            Format::Markdown,
            "answer.md",
            markdown_holds_the_whole_document_and_stands_alone,
        ),
        (
            Format::Pdf,
            "answer.pdf",
            pdf_holds_the_whole_document_with_its_pictures,
        ),
    ] {
        let path = folder.path().join(file);
        write(&support::answer_like(), format, &path).expect("the file is written");
        check(&path);
    }
}

fn html_holds_the_whole_document_typeset_and_standalone(path: &Path) {
    let html = support::text_of(path);
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>Black–Scholes formula and its assumptions</title>"));
    assert!(html.contains("<h1>Black–Scholes formula and its assumptions</h1>"));
    assert!(
        html.contains(
            "<p class=\"origin\">Question: How is the Black–Scholes formula derived?</p>"
        )
    );
    assert!(html.contains("<h2>Why the hedge removes risk</h2>"));
    assert!(html.contains("<em>partial differential equation</em>"));
    assert!(html.contains("<code>Delta</code>"));
    assert!(html.contains("<a class=\"cite\" href=\"#source-1\">[1]</a></p>"));
    assert!(html.contains(
        "<li id=\"source-2\">[2] Natenberg, chapter 1: Volatility, p. 5 — Figure 1.1</li>"
    ));

    // The inline formula and the formula set apart are pictures, and no LaTeX stays outside the
    // text that stands in for a picture.
    let formulas = html.matches("<img class=\"math\"").count();
    assert_eq!(formulas, 2);
    assert!(html.contains("<figure class=\"formula\"><img class=\"math\""));
    assert!(html.contains("<span class=\"label\">(2.3)</span>"));
    let mut without_alternatives = html.clone();
    for alternative in support::attribute_values(&html, "alt") {
        without_alternatives = without_alternatives.replace(&format!("alt=\"{alternative}\""), "");
    }
    assert!(!without_alternatives.contains("\\frac"));
    assert!(!without_alternatives.contains('$'));

    let figure = html
        .split("<img src=\"")
        .nth(1)
        .expect("the figure is a picture");
    let (address, rest) = figure.split_once('"').unwrap();
    assert!(rest.starts_with(" alt=\"A chart of volatility against time\">"));
    let picture = STANDARD
        .decode(
            address
                .strip_prefix("data:image/png;base64,")
                .expect("a data address"),
        )
        .expect("the picture is base64");
    assert_eq!(picture, std::fs::read(support::sample_picture()).unwrap());

    assert!(html.contains("<th>Greek</th>"));
    assert!(html.contains("<td>Delta</td>"));
    assert!(html.contains("<pre><code class=\"language-python\">def call_price(spot, strike):\n    return max(spot - strike, 0) if spot &gt; 0 else 0\n</code></pre>"));

    assert!(!html.contains("<link"));
    assert!(!html.contains("<script"));
    for address in support::attribute_values(&html, "src") {
        assert!(
            address.starts_with("data:"),
            "{address} must be inside the file"
        );
    }
    assert!(!html.contains("samples/content"));
    assert!(!html.contains("http"));
}

fn markdown_holds_the_whole_document_and_stands_alone(path: &Path) {
    let text = support::text_of(path);
    assert!(text.starts_with("# Black–Scholes formula and its assumptions\n\n> Question: How is the Black–Scholes formula derived?\n\n"));
    assert!(text.contains(
        "once the `Delta` hedge removes risk, where the volatility $\\sigma$ is constant. [1]\n\n"
    ));
    assert!(text.contains("\n## Why the hedge removes risk\n"));
    assert!(text.contains("\n$$\n\\frac{\\partial V}{\\partial t}"));
    assert!(text.contains("= 0\n$$\n\n(2.3) As printed in the source [1]\n"));
    assert!(text.contains(
        "\n**Table 1.1** Greeks [1]\n\n| Greek | Measures |\n| --- | --- |\n| Delta | Price change per unit of the stock |\n"
    ));
    assert!(text.contains(
        "\n```python\ndef call_price(spot, strike):\n    return max(spot - strike, 0) if spot > 0 else 0\n```\n\n**Listing 2.1** The call price in Python [2]\n"
    ));
    assert!(text.ends_with(
        "\n## Sources\n\n- [1] Hull, chapter 2: Options, p. 7\n- [2] Natenberg, chapter 1: Volatility, p. 5 — Figure 1.1\n"
    ));

    let figure = text
        .lines()
        .find(|line| {
            line.starts_with("![A chart of volatility against time](data:image/png;base64,")
        })
        .expect("the figure is in the file");
    let encoded = figure
        .split("base64,")
        .nth(1)
        .and_then(|rest| rest.strip_suffix(')'))
        .expect("the picture is a data address");
    let picture = STANDARD.decode(encoded).expect("the picture is base64");
    assert_eq!(picture, std::fs::read(support::sample_picture()).unwrap());
    assert!(text.contains(&format!(
        "{figure}\n\n**Figure 1.1** Volatility over time [2]\n"
    )));

    assert!(
        !text.contains("samples/content"),
        "the file names no path of the sample folder"
    );
    assert!(!text.contains("http"), "the file needs no network");
}

fn pdf_holds_the_whole_document_with_its_pictures(path: &Path) {
    assert!(std::fs::read(path).unwrap().starts_with(b"%PDF-"));
    let text = support::pdf_text(path);
    for phrase in [
        "Question:HowistheBlack–Scholesformuladerived?",
        "Black–Scholesformulaanditsassumptions",
        "Whythehedgeremovesrisk",
        "Thepriceofanoptionfollowsfromapartialdifferentialequation",
        "thevolatility",
        "isconstant.",
        "(2.3)",
        "Figure1.1Volatilityovertime",
        "Table1.1Greeks",
        "Delta",
        "Pricechangeperunitofthestock",
        "defcall_price(spot,strike):",
        "Listing2.1ThecallpriceinPython",
        "[1]",
        "[2]",
        "Sources",
        "Hull,chapter2:Options,p.7",
        "Natenberg,chapter1:Volatility,p.5—Figure1.1",
    ] {
        assert!(
            text.contains(phrase),
            "the PDF has no {phrase:?} in {text:?}"
        );
    }
    assert!(!text.contains("\\frac"), "formulas are pictures, not LaTeX");

    let sample = image::ImageReader::open(support::sample_picture())
        .unwrap()
        .into_dimensions()
        .unwrap();
    let pictures = support::pdf_pictures(path);
    assert!(pictures.contains(&(i64::from(sample.0), i64::from(sample.1))));
    let formulas = pictures
        .iter()
        .filter(|&&picture| picture != (i64::from(sample.0), i64::from(sample.1)))
        .count();
    assert!(
        formulas >= 2,
        "the inline formula and the one set apart are pictures"
    );
}
