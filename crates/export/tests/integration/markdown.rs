//! A document written as a Markdown file.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use export::{Format, write};

use crate::support;

#[test]
fn markdown_holds_the_whole_document_and_stands_alone() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("answer.md");

    write(&support::answer_like(), Format::Markdown, &path).expect("the Markdown is written");

    let text = support::text_of(&path);
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
