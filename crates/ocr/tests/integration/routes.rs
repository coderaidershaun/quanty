//! Checks on the route, the reasons and the paid calls that each page of the stubbed chapter run
//! saves.

use std::path::Path;

use ocr::testing::{Call, StubServices, page_folder, read_json};

fn strings(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect()
}

fn call_steps(page: &serde_json::Value) -> Vec<&str> {
    page["conversion"]["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["step"].as_str().unwrap())
        .collect()
}

pub fn assert_routes_reasons_and_calls(chapter: &Path, stubs: &StubServices) {
    let expected: [(&str, &[&str], &[&str]); 7] = [
        ("haiku-copy", &[], &["tag", "copy"]),
        (
            "haiku-then-sonnet",
            &["copy-flagged-stronger-model"],
            &["tag", "copy", "transcribe"],
        ),
        (
            "haiku-then-sonnet",
            &["copy-contains-backslash"],
            &["tag", "copy", "transcribe"],
        ),
        (
            "haiku-then-sonnet",
            &["copy-failed-copy-check"],
            &["tag", "copy", "transcribe", "transcribe"],
        ),
        (
            "haiku-then-sonnet",
            &["copy-failed-copy-check"],
            &["tag", "copy", "transcribe", "transcribe"],
        ),
        ("sonnet", &["tags-report-table"], &["tag", "transcribe"]),
        ("sonnet", &["math-check-failed"], &["tag", "transcribe"]),
    ];
    for (position, (route, reasons, steps)) in (1..=7).zip(expected) {
        let page = read_json(&page_folder(chapter, position).join("page.json"));
        assert_eq!(page["format-version"], 1);
        assert_eq!(page["page-position"], position);
        let shown = match position {
            6 => "9".to_owned(),
            7 => "10".to_owned(),
            other => other.to_string(),
        };
        assert_eq!(
            page["printed-page-number"],
            shown.as_str(),
            "page {position}"
        );
        let conversion = &page["conversion"];
        assert_eq!(conversion["route"], route, "page {position}");
        assert_eq!(
            strings(&conversion["route-reasons"]),
            reasons,
            "page {position}"
        );
        assert_eq!(conversion["tags"].as_object().unwrap().len(), 14);
        for ratio in ["piece-words-in-text-layer", "text-layer-words-in-pieces"] {
            assert!(
                conversion["checks"]["word-match"][ratio].is_number(),
                "page {position} lacks {ratio}"
            );
        }
        assert_eq!(call_steps(&page), steps, "page {position}");
        for call in page["conversion"]["calls"].as_array().unwrap() {
            assert_eq!(call["input-tokens"], 100, "page {position}: {call}");
        }
        assert_eq!(
            stubs.calls_for(position).len(),
            steps.len() + 1,
            "page {position}: the math check is a call too"
        );
    }
    let page_seven = read_json(&page_folder(chapter, 7).join("page.json"));
    assert_eq!(page_seven["conversion"]["math-check"], "no-answer");
    let failure = page_seven["conversion"]["math-check-failure"]
        .as_str()
        .expect("page 7 should say why its math check gave no answer");
    assert!(
        failure.contains("status 503") && failure.contains("stub outage"),
        "{failure}"
    );
    let page_six = read_json(&page_folder(chapter, 6).join("page.json"));
    assert!(page_six["conversion"]["math-check-failure"].is_null());
    assert_eq!(
        stubs.calls_for(1),
        [Call::Tag(1), Call::Math(1), Call::Copy(1)]
    );
    assert_eq!(stubs.calls().len(), 27);
}
