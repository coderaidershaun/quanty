//! A document that no answer is behind, written in every format.

use export::{Format, insights_document, write};

use crate::support;

#[test]
fn a_cheat_sheet_with_no_answer_is_written_in_every_format() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let sheet = support::cheat_sheet();

    let markdown_path = folder.path().join("sheet.md");
    write(&sheet, Format::Markdown, &markdown_path).expect("the Markdown is written");
    let markdown = support::text_of(&markdown_path);
    assert!(markdown.starts_with("# Options cheat sheet\n\n> Hull, chapter 2: Options\n\n"));
    assert!(markdown.contains("- A call pays $\\max(S - K, 0)$ at the end\n- A put gives"));
    assert!(markdown.contains("## Put–call parity\n\n"));
    assert!(markdown.contains("\n\n$$C - P = S - K e^{-rT}$$\n\n"));
    assert!(
        markdown.contains("| Symbol | Meaning |\n| --- | --- |\n| $S$ | the stock price now |")
    );
    assert!(!markdown.contains("Sources"));

    let html_path = folder.path().join("sheet.html");
    write(&sheet, Format::Html, &html_path).expect("the HTML is written");
    let html = support::text_of(&html_path);
    assert!(html.contains("<li>A put gives the right to sell at the strike</li>"));
    assert!(html.contains("<li>Delta is the price change per unit of the stock</li>"));
    assert!(html.contains("<h2>Put–call parity</h2>"));
    assert!(html.contains("<table>"));
    assert!(html.contains("<th>Symbol</th>"));
    // A point of the list, two formulas in the paragraph, the formula set apart, and the two
    // symbols of the table.
    assert_eq!(html.matches("<img class=\"math\"").count(), 6);
    assert!(!html.contains("Sources"));

    let pdf_path = folder.path().join("sheet.pdf");
    write(&sheet, Format::Pdf, &pdf_path).expect("the PDF is written");
    let text = support::pdf_text(&pdf_path);
    for phrase in [
        "Optionscheatsheet",
        "Hull,chapter2:Options",
        "Aputgivestherighttosellatthestrike",
        "Deltaisthepricechangeperunitofthestock",
        "Put–callparity",
        "Symbol",
        "Meaning",
        "thestockpricenow",
    ] {
        assert!(
            text.contains(phrase),
            "the PDF has no {phrase:?} in {text:?}"
        );
    }
    assert!(!text.contains("Sources"));
    assert!(!text.contains("\\"), "formulas are pictures, not LaTeX");
    assert!(
        support::pdf_pictures(&pdf_path).len() >= 6,
        "one picture for each formula"
    );
}

#[test]
fn the_insights_of_a_chapter_are_written_in_every_format() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let sheet = support::insights_sheet();
    let document = insights_document(&sheet);

    let markdown_path = folder.path().join("insights.md");
    write(&document, Format::Markdown, &markdown_path).expect("the Markdown is written");
    let markdown = support::text_of(&markdown_path);
    assert!(
        markdown.starts_with(
            "# Hull, chapter 2: Options\n\n> Insights, takeaways and cheat sheet written by sonnet\n\n## Insights\n\n"
        ),
        "the Markdown starts with {markdown:?}"
    );
    assert!(markdown.contains("has no random part left.\n\n*Pages 1 and 3*\n\n"));
    assert!(markdown.contains("to rise.\n\n## Takeaways\n\n"));
    assert!(markdown.contains("- A hedged portfolio is riskless\n- A riskless portfolio earns"));
    assert!(markdown.contains("\n\n## Cheat sheet\n\n### Put–call parity\n\n"));
    assert!(markdown.contains("\n\n$$C - P = S - K e^{-rT}$$\n\n"));
    assert!(
        markdown.contains("| Symbol | Meaning |\n| --- | --- |\n| $S$ | the stock price now |")
    );
    assert!(!markdown.contains("Sources"));

    let html_path = folder.path().join("insights.html");
    write(&document, Format::Html, &html_path).expect("the HTML is written");
    let html = support::text_of(&html_path);
    assert!(html.contains("<h2>Insights</h2>"));
    assert!(html.contains("<h3>Put–call parity</h3>"));
    assert!(html.contains("<table>"));
    // The formula of the first insight, the one of the second takeaway, the one set apart, and
    // the two symbols of the table.
    assert_eq!(html.matches("<img class=\"math\"").count(), 5);
    assert!(!html.contains("Sources"));

    let pdf_path = folder.path().join("insights.pdf");
    write(&document, Format::Pdf, &pdf_path).expect("the PDF is written");
    let text = support::pdf_text(&pdf_path);
    for phrase in [
        "Hull,chapter2:Options",
        "Insights,takeawaysandcheatsheetwrittenbysonnet",
        "Takeaways",
        "Cheatsheet",
        "thestockpricenow",
    ] {
        assert!(
            text.contains(phrase),
            "the PDF has no {phrase:?} in {text:?}"
        );
    }
    assert!(!text.contains("\\"), "formulas are pictures, not LaTeX");
    assert!(
        support::pdf_pictures(&pdf_path).len() >= 5,
        "one picture for each formula"
    );
}
