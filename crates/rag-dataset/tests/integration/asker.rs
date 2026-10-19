//! The asker that every model-written part of a dataset goes through: it reads an answer it has
//! kept, asks a failed question once more, stops at a failure that no question could pass, and asks
//! a few at a time.

use rag_core::LlmError;
use rag_dataset::generate::asker::{Answer, AskError, Asker};
use rag_dataset::generate::role::Role;
use rag_ingestion::testing::StandInLlm;
use serde::Deserialize;
use serde_json::json;

const SHORTEN: Role = Role {
    id: "shorten",
    version: 1,
    system_prompt: "Shorten the text.",
    schema: r#"{"type": "object", "required": ["text"]}"#,
};

#[derive(Debug, Deserialize, PartialEq)]
struct Shortened {
    text: String,
}

fn shortened(text: &str) -> Answer<Shortened> {
    Answer::Given(Shortened {
        text: text.to_owned(),
    })
}

/// Answers every question with the input in front of "short:".
fn shortening() -> StandInLlm {
    StandInLlm::replying(|input, _| Ok(json!({ "text": format!("short: {input}") })))
}

fn timed_out() -> LlmError {
    LlmError::TimedOut { seconds: 120 }
}

#[tokio::test]
async fn a_second_identical_ask_is_answered_from_the_cache() {
    let folder = tempfile::tempdir().unwrap();
    let llm = shortening();
    let asker = Asker::new(&llm, folder.path()).unwrap();

    let first = asker.ask::<Shortened>(&SHORTEN, "long text").await.unwrap();

    assert_eq!(first, shortened("short: long text"));
    assert_eq!(llm.calls(), 1);
    let paid_for = asker.usage();
    assert_eq!(
        paid_for.tokens().input_tokens,
        1000,
        "the stand-in says 1,000 read"
    );
    let question = &llm.questions()[0];
    assert_eq!(question.system_prompt, SHORTEN.system_prompt);
    assert_eq!(question.schema, SHORTEN.schema);
    assert_eq!(question.input, "long text");

    let second = asker.ask::<Shortened>(&SHORTEN, "long text").await.unwrap();

    assert_eq!(second, shortened("short: long text"));
    assert_eq!(
        llm.calls(),
        1,
        "the kept answer was read, and the model was not asked"
    );
    assert_eq!(
        asker.usage(),
        paid_for,
        "an answer that was read cost nothing"
    );
    assert_eq!((asker.calls(), asker.cache_hits()), (1, 1));

    let reopened = Asker::new(&llm, folder.path()).unwrap();
    let third = reopened
        .ask::<Shortened>(&SHORTEN, "long text")
        .await
        .unwrap();

    assert_eq!(third, shortened("short: long text"));
    assert_eq!(
        llm.calls(),
        1,
        "a new asker over the same folder reads the same answers"
    );
    assert!(reopened.usage().is_empty());
    assert_eq!((reopened.calls(), reopened.cache_hits()), (0, 1));

    let revised = Role {
        version: 2,
        ..SHORTEN
    };
    reopened
        .ask::<Shortened>(&revised, "long text")
        .await
        .unwrap();
    assert_eq!(
        llm.calls(),
        2,
        "a new version of the prompt is a new question"
    );
}

#[tokio::test]
async fn a_failed_question_is_asked_once_more() {
    let folder = tempfile::tempdir().unwrap();

    let flaky = StandInLlm::replying(|_, earlier| match earlier {
        0 => Err(timed_out()),
        _ => Ok(json!({ "text": "second time" })),
    });
    let asker = Asker::new(&flaky, folder.path()).unwrap();
    let answer = asker.ask::<Shortened>(&SHORTEN, "flaky").await.unwrap();
    assert_eq!(answer, shortened("second time"));
    assert_eq!(flaky.calls(), 2);
    assert_eq!(asker.calls(), 2);
    assert_eq!(
        asker.usage().tokens().input_tokens,
        1000,
        "a question that failed reported no usage"
    );

    let down = StandInLlm::replying(|_, _| Err(timed_out()));
    let asker = Asker::new(&down, folder.path()).unwrap();
    let Answer::Failed { reason } = asker.ask::<Shortened>(&SHORTEN, "down").await.unwrap() else {
        panic!("a question that fails twice should give no answer");
    };
    assert!(
        reason.contains("did not answer within 120 seconds"),
        "{reason}"
    );
    assert_eq!(down.calls(), 2);

    let wrong_shape = StandInLlm::replying(|_, _| Ok(json!({ "unexpected": 1 })));
    let asker = Asker::new(&wrong_shape, folder.path()).unwrap();
    let Answer::Failed { reason } = asker
        .ask::<Shortened>(&SHORTEN, "wrong shape")
        .await
        .unwrap()
    else {
        panic!("a reply that does not fit should count as a failed try");
    };
    assert!(reason.contains("missing field `text`"), "{reason}");
    assert_eq!(wrong_shape.calls(), 2);
    assert_eq!(
        asker.usage().tokens().input_tokens,
        2000,
        "both replies were paid for, though neither fitted"
    );
}

#[tokio::test]
async fn a_question_that_no_question_could_pass_stops_the_run() {
    let folder = tempfile::tempdir().unwrap();

    let at_limit = StandInLlm::replying(|_, _| {
        Err(LlmError::UsageLimit {
            message: "resets at 5pm".to_owned(),
        })
    });
    let asker = Asker::new(&at_limit, folder.path()).unwrap();
    let result = asker.ask::<Shortened>(&SHORTEN, "any").await;
    assert!(
        matches!(result, Err(AskError::Stopped(LlmError::UsageLimit { .. }))),
        "{result:?}"
    );
    assert_eq!(at_limit.calls(), 1, "a stop is not asked again");

    let signed_out = StandInLlm::signed_out();
    let asker = Asker::new(&signed_out, folder.path()).unwrap();
    let result = asker.ask::<Shortened>(&SHORTEN, "any").await;
    assert!(
        matches!(result, Err(AskError::Stopped(LlmError::NotSignedIn { .. }))),
        "{result:?}"
    );
    assert_eq!(signed_out.calls(), 1);
}

#[tokio::test]
async fn ask_each_asks_at_most_four_at_once_in_order() {
    let folder = tempfile::tempdir().unwrap();
    let llm = shortening();
    let asker = Asker::new(&llm, folder.path()).unwrap();
    let inputs: Vec<String> = (1..=10).map(|number| format!("text {number}")).collect();

    let answers = asker
        .ask_each::<Shortened>(&SHORTEN, &inputs)
        .await
        .unwrap();

    let expected: Vec<_> = inputs
        .iter()
        .map(|input| shortened(&format!("short: {input}")))
        .collect();
    assert_eq!(
        answers, expected,
        "the answers come back in the order of the inputs"
    );
    assert_eq!(llm.calls(), 10);
    assert_eq!(llm.most_calls_at_once(), 4);
}
