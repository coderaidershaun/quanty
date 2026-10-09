//! Checks on what the stubbed chapter run reports: its summary, and the progress it tells for each
//! page.

use std::collections::BTreeMap;

use ocr::ConversionSummary;
use ocr::convert::PageProgress;

pub fn assert_summary_counts_and_lists(summary: &ConversionSummary) {
    assert_eq!(
        (
            summary.calls.tag,
            summary.calls.math_check,
            summary.calls.copy,
            summary.calls.transcribe
        ),
        (7, 7, 5, 8)
    );
    assert_eq!(
        (
            summary.routes.haiku_copy,
            summary.routes.sonnet,
            summary.routes.haiku_then_sonnet
        ),
        (1, 2, 4)
    );
    let printed = summary.to_string();
    assert!(
        printed.contains("printed page numbers out of sequence: page 6 shows 9\n"),
        "{printed}"
    );
    assert!(
        printed
            .contains("tokens of stub-transcriber: 800 in, 80 out, 0 cache read, 0 cache write\n"),
        "{printed}"
    );
    let to_check = printed
        .lines()
        .find(|line| line.starts_with("pages to check:"))
        .unwrap();
    assert!(!to_check.contains("page 1 "), "{to_check}");
    for position in 2..=7 {
        assert!(
            to_check.contains(&format!("page {position} (low word match")),
            "{to_check}"
        );
    }
    for expected in [
        "page 2 (low word match, figure image was not checked against the page's text)",
        "page 3 (low word match, figure image is most of the page, figure image holds body text, figure image was not checked against the page's text)",
        "page 4 (low word match, figure image is the whole page)",
        "page 5 (low word match, figure image is the whole page)",
    ] {
        assert!(
            to_check.contains(expected),
            "{expected} missing from {to_check}"
        );
    }
}

/// Every page is told cut out, in order, before the first one is told saved. Pages end in any
/// order, so the saved positions are sorted before they are compared.
pub fn assert_each_page_told_once_with_its_cost(
    heard: &[PageProgress],
    summary: &ConversionSummary,
) {
    assert_eq!(
        heard.first(),
        Some(&PageProgress::Pages {
            total: 7,
            done_before: 0
        })
    );
    let cut: Vec<u32> = heard[1..]
        .iter()
        .map_while(|progress| match progress {
            PageProgress::PageCut { position } => Some(*position),
            _ => None,
        })
        .collect();
    assert_eq!(cut, (1..=7).collect::<Vec<u32>>());
    let mut positions = Vec::new();
    let mut cost_usd = 0.0;
    // Input tokens, output tokens and cents, for each model.
    let mut told: BTreeMap<&str, [u64; 3]> = BTreeMap::new();
    let cents = |usd: f64| (usd * 100.0).round() as u64;
    for progress in &heard[1 + cut.len()..] {
        match progress {
            PageProgress::PageDone { position, calls } => {
                positions.push(*position);
                for (model, used) in &calls.by_model {
                    cost_usd += used.cost_usd;
                    let sum = told.entry(model.as_str()).or_default();
                    sum[0] += used.input_tokens;
                    sum[1] += used.output_tokens;
                    sum[2] += cents(used.cost_usd);
                }
            }
            other => panic!("only finished pages should follow the cut pages, got {other:?}"),
        }
    }
    positions.sort_unstable();
    assert_eq!(positions, (1..=7).collect::<Vec<u32>>());
    let in_summary: BTreeMap<&str, [u64; 3]> = (summary.calls.by_model.iter())
        .map(|(model, used)| {
            let figures = [used.input_tokens, used.output_tokens, cents(used.cost_usd)];
            (model.as_str(), figures)
        })
        .collect();
    let expected = [
        ("stub-copier", [500, 50, 5]),
        ("stub-tagger", [700, 70, 7]),
        ("stub-transcriber", [800, 80, 8]),
    ];
    assert_eq!(in_summary, BTreeMap::from(expected));
    assert_eq!(
        told, in_summary,
        "the pages tell the tokens and the cost of the run"
    );
    let reported = summary.calls.by_model.values().map(|used| used.cost_usd);
    let summary_cost: f64 = reported.sum();
    assert!(
        (cost_usd - summary_cost).abs() < 1e-9,
        "the pages told {cost_usd}, the summary says {summary_cost}"
    );
}
