//! Checks that the rules the desktop app keeps a copy of still agree with the crates that own them,
//! which the contract and the panels may not name.

use egui_kittest::kittest::Queryable;
use gui::contract::{self, Answer, ModelTokens, Usage};
use gui::panels::answer;
use gui::testkit::{self, sample};
use rag_dataset::options::{DEFAULT_MAX_SFT, DEFAULT_QUESTIONS, DEFAULT_REWRITES};

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
fn the_contract_tells_which_documents_get_insights_as_the_converter_does() {
    let names = [
        (
            contract::DocumentName::Chapter {
                number: 2,
                name: "Greeks".to_owned(),
            },
            ocr::DocumentName::Chapter {
                number: 2,
                name: "Greeks".to_owned(),
            },
        ),
        (
            contract::DocumentName::Title("Index".to_owned()),
            ocr::DocumentName::Title("Index".to_owned()),
        ),
    ];
    let categories = [
        (
            Some(contract::Category::Book),
            Some(rag_core::Category::Book),
        ),
        (
            Some(contract::Category::Paper),
            Some(rag_core::Category::Paper),
        ),
        (
            Some(contract::Category::Other),
            Some(rag_core::Category::Other),
        ),
        (None, None),
    ];
    for (name, converter_name) in &names {
        for (category, converter_category) in categories {
            assert_eq!(
                name.gets_insights(category),
                ocr::gets_insights(converter_name, converter_category),
                "{name:?} in {category:?}"
            );
        }
    }
}

#[test]
fn the_dataset_form_starts_at_the_defaults_of_the_dataset_crate() {
    let form = contract::DatasetRequest::default();
    assert_eq!(form.rewrites, DEFAULT_REWRITES);
    assert_eq!(form.questions, DEFAULT_QUESTIONS);
    assert_eq!(form.max_sft, DEFAULT_MAX_SFT);
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
        let cost_words = rag_core::cost_text(Some(cost));
        let worded = match cost_words.strip_prefix("≈ ") {
            Some(price) => format!("12 tokens, about {price}"),
            None => format!("12 tokens, {cost_words}"),
        };
        assert!(
            harness.query_by_label(&worded).is_some(),
            "the answer header should say {worded:?}"
        );
    }
}
