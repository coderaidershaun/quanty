//! A document written as an HTML file.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use export::{Format, write};

use crate::support;

#[test]
fn html_holds_the_whole_document_typeset_and_standalone() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("answer.html");

    write(&support::answer_like(), Format::Html, &path).expect("the HTML is written");

    let html = support::text_of(&path);
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
