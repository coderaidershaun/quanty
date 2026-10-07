//! Converts the sample chapter with the real models and the real Jev API, because nothing
//! offline can tell whether the flags, schemas and prompts still work together.

use ocr::content::PieceDetail;
use ocr::testing::{read_json, sample_job};
use ocr::{convert_chapter, read_chapter};

use crate::read_chapter::assert_sample_chapter;

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p ocr --test integration -- --ignored convert_live::converts_sample_chapter_live --nocapture";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real models and Jev and spends quota; run it with: {RUN_COMMAND}"
    );
}

/// Which way the plain text page goes is not fixed on a live run: the tagger sometimes takes its
/// shaded corner block for a picture, and that rightly sends it to Sonnet.
fn assert_route_follows_its_reasons(position: u32, page: &serde_json::Value) {
    let conversion = &page["conversion"];
    let route = conversion["route"].as_str().unwrap();
    let reasons = conversion["route-reasons"].as_array().unwrap();
    let steps: Vec<&str> = conversion["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["step"].as_str().unwrap())
        .collect();
    if position == 1 {
        println!("page 1 route: {route}, reasons: {reasons:?}");
    }
    if reasons.is_empty() {
        assert_eq!(route, "haiku-copy", "page {position}");
        assert!(steps.contains(&"copy"), "page {position}: {steps:?}");
        assert!(!steps.contains(&"transcribe"), "page {position}: {steps:?}");
    } else {
        assert!(
            matches!(route, "sonnet" | "haiku-then-sonnet"),
            "page {position}: {route}"
        );
        assert!(steps.contains(&"transcribe"), "page {position}: {steps:?}");
    }
}

/// Nothing is asserted here because a live rectangle varies from run to run. It is printed first
/// so a run that fails on a figure still shows what it gave.
fn print_figures(chapter: &std::path::Path) {
    for piece in read_chapter(chapter).unwrap().pieces {
        if let PieceDetail::Figure {
            label,
            bounds,
            image: Some(image),
            ..
        } = &piece.detail
        {
            println!(
                "figure {label:?} on page {}: bounds {bounds:?}, cut {:?}, shows {:?}, holds body text {}, unchecked {}",
                piece.id.page, image.cut, image.shows, image.holds_body_text, image.unchecked
            );
        }
    }
}

#[tokio::test]
#[ignore = "calls the real claude CLI and the real Jev API and spends quota; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p ocr --test integration -- --ignored convert_live::converts_sample_chapter_live --nocapture"]
async fn converts_sample_chapter_live() {
    require_prod_api();
    let root = tempfile::tempdir().unwrap();
    let job = sample_job(root.path());

    let summary = convert_chapter(&job)
        .await
        .unwrap_or_else(|error| panic!("the live conversion failed: {error:#?}"));
    println!("{summary}");

    let chapter = job.chapter_folder();
    print_figures(&chapter);
    assert_sample_chapter(&chapter);
    for position in 1..=7 {
        let page = read_json(&chapter.join(format!("page-num-{position}/page.json")));
        let calls = page["conversion"]["calls"].as_array().unwrap();
        assert_eq!(calls[0]["step"], "tag", "page {position}");
        for call in calls {
            assert!(
                call["model"]
                    .as_str()
                    .is_some_and(|model| !model.is_empty()),
                "page {position}: {call}"
            );
        }
        assert_route_follows_its_reasons(position, &page);
    }

    let again = convert_chapter(&job).await.unwrap();
    assert_eq!(
        again.calls.tag + again.calls.math_check + again.calls.copy + again.calls.transcribe,
        0
    );
}
