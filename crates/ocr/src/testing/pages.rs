//! The canned pages the stand-ins answer with: one standard page, and the changes that send each
//! page of the sample chapter down a different branch of the run.

use crate::content::{PageBox, Symbol};
use crate::convert::reply::{
    CitedKind, CitedLabel, CopiedPage, CopiedPiece, Discussion, TranscribedPage, TranscribedPiece,
};

/// The canned figure's rectangle. Its left edge is 0, so the padding is clamped at the page edge;
/// the other three sides show the padding.
const CANNED_FIGURE_BOUNDS: PageBox = PageBox {
    left: 0,
    top: 200,
    right: 700,
    bottom: 500,
};

/// The page's position, except that page 6 shows 9 and page 7 shows 10, so exactly one page is
/// out of sequence.
fn printed_number(position: u32) -> Option<String> {
    Some(match position {
        6 => "9".to_owned(),
        7 => "10".to_owned(),
        other => other.to_string(),
    })
}

fn text(number: u32, markdown: &str, cites: Vec<CitedLabel>) -> TranscribedPiece {
    TranscribedPiece::Text {
        number,
        markdown: markdown.to_owned(),
        cites,
    }
}

fn formula(number: u32, latex: &str, label: &str) -> TranscribedPiece {
    TranscribedPiece::Formula {
        number,
        latex: latex.to_owned(),
        label: Some(label.to_owned()),
        name: None,
        statement: "The first quantity equals the second plus the third.".to_owned(),
        symbols: vec![Symbol {
            symbol: "a".to_owned(),
            meaning: "the first quantity".to_owned(),
        }],
    }
}

fn page_of(position: u32, pieces: Vec<TranscribedPiece>) -> TranscribedPage {
    TranscribedPage {
        printed_page_number: printed_number(position),
        running_header: None,
        pieces,
        discusses: Vec::new(),
        starts_mid_sentence: false,
        ends_mid_sentence: false,
    }
}

// Keep this page small. Its copied words are compared with the text layer of every sample page,
// and the tests expect that match to come out low on all of them. That only holds while the
// words copied here stay under about 150.
pub(super) fn standard_transcription(position: u32) -> TranscribedPage {
    let pieces = vec![
        TranscribedPiece::Heading {
            number: 1,
            rank: 2,
            printed_number: None,
            text: "A Canned Heading".to_owned(),
        },
        text(
            2,
            "The price is given by",
            vec![CitedLabel {
                kind: CitedKind::Equation,
                label: "(2.1)".to_owned(),
            }],
        ),
        formula(3, "a = b + c", "(2.1)"),
        text(
            4,
            "The chart shows the result and the term is noted.[^1]",
            vec![CitedLabel {
                kind: CitedKind::Figure,
                label: "Figure 3-1".to_owned(),
            }],
        ),
        canned_figure(
            5,
            "Figure 3-1",
            "Growth.",
            &["Growth", "Time"],
            CANNED_FIGURE_BOUNDS,
        ),
        TranscribedPiece::Table {
            number: 6,
            label: Some("Table 3-1".to_owned()),
            caption: None,
            markdown: "| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |".to_owned(),
            note: None,
            summary: "One row of three numbers.".to_owned(),
        },
        TranscribedPiece::Footnote {
            number: 7,
            marker: Some("1".to_owned()),
            markdown: "A note about the term.".to_owned(),
            cites: Vec::new(),
        },
    ];
    TranscribedPage {
        discusses: vec![Discussion { piece: 4, about: 5 }],
        ..page_of(position, pieces)
    }
}

fn canned_figure(
    number: u32,
    label: &str,
    caption: &str,
    printed_text: &[&str],
    bounds: PageBox,
) -> TranscribedPiece {
    TranscribedPiece::Figure {
        number,
        label: Some(label.to_owned()),
        caption: Some(caption.to_owned()),
        printed_text: printed_text.iter().map(|text| (*text).to_owned()).collect(),
        bounds,
        explanation: "A line chart with time along the bottom axis and growth up the side. The \
            line starts low on the left, climbs steadily through the middle of the chart and \
            flattens near the top right. The title reads Growth and the bottom axis is labelled \
            Time. The chart shows growth that slows over time, so early gains are large and \
            later gains are small, and the line never turns down anywhere on the page."
            .to_owned(),
    }
}

const UNUSABLE_BOUNDS: PageBox = PageBox {
    left: 700,
    top: 200,
    right: 100,
    bottom: 500,
};

// Lines really printed on pages 3 and 6. The canned text repeats them so `ocr` can tell they
// are body text when it works out where to cut a figure.
const PAGE_THREE_LINE: &str = "a trader depending on the types of strategies being executed";
const PAGE_SIX_LINES: &str = "gamma, the potential profit when the underlying market moves. The risk is the theta, the money that will be lost through the passage of time";

fn add_sentence(mut page: TranscribedPage, sentence: &str) -> TranscribedPage {
    if let TranscribedPiece::Text { markdown, .. } = &mut page.pieces[3] {
        markdown.push(' ');
        markdown.push_str(sentence);
    }
    page
}

fn set_figure_bounds(mut page: TranscribedPage, bounds: PageBox) -> TranscribedPage {
    if let TranscribedPiece::Figure { bounds: old, .. } = &mut page.pieces[4] {
        *old = bounds;
    }
    page
}

fn break_formula(mut page: TranscribedPage) -> TranscribedPage {
    if let TranscribedPiece::Formula { latex, .. } = &mut page.pieces[2] {
        *latex = "\\frac{a".to_owned();
    }
    page
}

pub(super) fn sample_chapter_transcription(position: u32, is_second_try: bool) -> TranscribedPage {
    let canned = standard_transcription(position);
    let rectangle = |left, top, right, bottom| PageBox {
        left,
        top,
        right,
        bottom,
    };
    match (position, is_second_try) {
        (3, _) => add_sentence(
            set_figure_bounds(canned, rectangle(0, 50, 1000, 950)),
            PAGE_THREE_LINE,
        ),
        (4, false) => break_formula(canned),
        (4, true) | (5, false) => set_figure_bounds(canned, UNUSABLE_BOUNDS),
        // This reply has a bad rectangle and a broken formula. It must never be saved as a reply
        // whose only fault is its rectangle.
        (5, true) => break_formula(set_figure_bounds(canned, UNUSABLE_BOUNDS)),
        (6, _) => {
            let mut page = add_sentence(canned, PAGE_SIX_LINES);
            page.pieces[4] = canned_figure(
                5,
                "Figure 13-16",
                "Dividend sensitivity.",
                &["Expected quarterly dividend"],
                rectangle(115, 55, 950, 675),
            );
            TranscribedPage {
                ends_mid_sentence: true,
                ..page
            }
        }
        (7, _) => {
            let mut page = page_seven_transcription();
            page.pieces.push(canned_figure(
                3,
                "Figure 24-12",
                "FTSE 100 volatility surface. March 16. 2012.",
                &["Exercise price", "Months to expiration"],
                rectangle(80, 578, 950, 945),
            ));
            page
        }
        _ => canned,
    }
}

fn page_seven_transcription() -> TranscribedPage {
    page_of(
        7,
        vec![
            formula(1, "x = y", "(7.9)"),
            text(2, "where the symbols are as before", Vec::new()),
        ],
    )
}

pub(super) fn broken_transcription(position: u32) -> TranscribedPage {
    page_of(position, vec![formula(1, "\\frac{a", "(1.1)")])
}

pub(super) fn copy_page_of(position: u32, words: &str) -> CopiedPage {
    CopiedPage {
        needs_stronger_model: false,
        printed_page_number: printed_number(position),
        running_header: None,
        pieces: vec![CopiedPiece::Text {
            number: 1,
            markdown: words.to_owned(),
            cites: Vec::new(),
        }],
        starts_mid_sentence: false,
        ends_mid_sentence: false,
    }
}
