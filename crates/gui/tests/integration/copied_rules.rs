//! Checks that the rules the desktop app keeps a copy of still agree with the crates that own them,
//! which the contract and the panels may not name.

use egui_kittest::kittest::Queryable;
use gui::contract::{self, Answer, ModelTokens, Usage};
use gui::panels::answer;
use gui::testkit::{self, sample};

#[test]
fn the_contract_compares_names_as_the_core_crate_does() {
    let pairs = [
        ("Options", "options"),
        ("  Options ", "OPTIONS"),
        ("Options", "Option"),
        ("Straße", "STRASSE"),
        ("ÉCOLE", "école"),
        ("a  b", "a b"),
        ("", " "),
    ];
    for (one, other) in pairs {
        assert_eq!(
            contract::is_same_name(one, other),
            rag_core::is_same_name(one, other),
            "{one:?} and {other:?}"
        );
    }
}

#[test]
fn the_contract_reads_a_chapter_file_name_as_the_converter_does() {
    let names = [
        "chapter-1-sample-pages.pdf",
        "chapter-12-black-scholes--model.pdf",
        "chapter-03-greeks.pdf",
        "chapter-1-.pdf",
        "chapter-1.pdf",
        "chapter--greeks.pdf",
        "chapter-x-greeks.pdf",
        "chapter-99999999999-greeks.pdf",
        "Chapter-1-greeks.pdf",
        "chapter-1-greeks.PDF",
        "notes.pdf",
    ];
    for name in names {
        let converter = ocr::content::parse_chapter_file_name(name)
            .ok()
            .map(|read| match read {
                ocr::DocumentName::Chapter { number, name } => {
                    contract::DocumentName::Chapter { number, name }
                }
                ocr::DocumentName::Title(title) => contract::DocumentName::Title(title),
            });
        assert_eq!(
            contract::DocumentName::from_chapter_file_name(name),
            converter,
            "{name}"
        );
    }
}

#[test]
fn a_panel_words_a_cost_as_the_core_crate_does() {
    for cost in [0.0, 0.004, 0.005, 0.416, 12.345] {
        let usage = Usage {
            models: vec![ModelTokens {
                model: "claude-sonnet-5-5".to_owned(),
                input: 12,
                ..ModelTokens::default()
            }],
            cost_usd: Some(cost),
        };
        let shared = testkit::answered(
            sample::search_reply(),
            sample::concept_graph(),
            Answer {
                usage,
                ..sample::answer()
            },
        );
        let mut local = answer::Local::default();
        let mut harness = testkit::panel([900.0, 600.0], shared, move |ui, cx| {
            answer::show(ui, &mut local, cx);
        });
        harness.run();
        let worded = format!("12 tokens · {}", rag_core::cost_text(Some(cost)));
        assert!(
            harness.query_by_label(&worded).is_some(),
            "the answer header should say {worded:?}"
        );
    }
}
