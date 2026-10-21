//! A code listing among the results of an ask: the Code tab counts it, the list, the answer and
//! Share show it with its language and its indentation, and Copy code gives it as it is stored.

use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use gui::contract::{AnswerBlock, Intent, ItemId, ItemKind};
use gui::panels::answer;
use gui::testkit::{self, sample};
use uuid::Uuid;

const CODE: &str = "def call_price(spot, strike):\n    payoff = spot - strike\n\n    if payoff < 0:\n        return 0\n    return payoff";

#[test]
fn a_code_result_has_its_tab_its_row_its_card_and_its_fence_in_share() {
    let mut reply = sample::search_reply();
    let mut listing = reply.results[0].clone();
    listing.number = 4;
    listing.id = ItemId(Uuid::from_u128(104));
    listing.kind = ItemKind::Code;
    listing.label = Some("Listing 2.1".to_owned());
    listing.caption = Some("The call price in Python".to_owned());
    listing.language = Some("python".to_owned());
    listing.last_page = Some(5);
    listing.last_printed_page = Some("9".to_owned());
    listing.name = None;
    listing.text = CODE.to_owned();
    reply.results.push(listing);
    let mut written = sample::answer();
    written.blocks.push(AnswerBlock::Paragraph {
        text: "A short function prices the call.".to_owned(),
        cites: vec![4],
    });
    written.blocks.push(AnswerBlock::Item(4));
    let shared = testkit::answered(reply, sample::concept_graph(), written);
    let shared_markdown = shared.ask.answer_markdown().expect("the answer is written");
    let mut local = answer::Local::default();
    let mut harness = testkit::panel([900.0, 2400.0], shared, move |ui, cx| {
        answer::show(ui, &mut local, cx);
    });
    harness.run();

    assert_eq!(
        harness
            .get_by_role_and_label(Role::Tab, "Code")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("1")
    );
    assert!(
        harness.query_by_label("python").is_some(),
        "the card of the answer names the language"
    );
    assert!(
        harness.query_by_label(CODE).is_some(),
        "the card of the answer shows the code, indented as printed"
    );

    harness.get_by_label(CODE).click_secondary();
    harness.run();
    harness.get_by_label("Copy code").click();
    harness.run();
    assert_eq!(
        harness.state().intents.last(),
        Some(&Intent::CopyText(CODE.to_owned())),
        "Copy code copies the code as it is stored"
    );

    harness.get_by_role_and_label(Role::Tab, "Code").click();
    harness.run();

    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Result 4")
            .is_some()
    );
    for other in ["Result 1", "Result 2", "Result 3"] {
        assert!(
            harness
                .query_by_role_and_label(Role::Button, other)
                .is_none(),
            "the Code tab lists no row but the code: {other}"
        );
    }
    assert!(
        harness.query_by_label("python").is_some(),
        "the row names the language"
    );
    assert!(
        harness.query_by_label(CODE).is_some(),
        "the row shows the code, indented as printed"
    );
    assert!(
        harness.query_by_label_contains("pp. 7\u{2013}9").is_some(),
        "the row names the first and the last page of the listing"
    );

    assert!(
        shared_markdown.contains(&format!("*Listing 2.1* [4]\n\n```python\n{CODE}\n```")),
        "Share fences the code with its language:\n{shared_markdown}"
    );
    assert!(
        shared_markdown
            .contains("[4] Quanty Sample Notes, chapter 2: Black Scholes In Depth, pp. 7\u{2013}9"),
        "Share names the first and the last page of the listing:\n{shared_markdown}"
    );
}
