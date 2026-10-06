//! Calls the real `claude` program, because only the real program can show that it still takes
//! the flags of the lock-down and still answers in `structured_output`.

use rag_core::{ClaudeCli, Llm, Question};

const RUN_COMMAND: &str = "set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-core --test integration -- --ignored claude_live::";

fn require_prod_api() {
    assert_eq!(
        std::env::var("REX_PROD_API").as_deref(),
        Ok("true"),
        "this test calls the real claude program; run it with: {RUN_COMMAND}"
    );
}

#[tokio::test]
#[ignore = "calls the real claude program once and spends subscription usage; run with: set -a; . ./.env; set +a; REX_PROD_API=true cargo test -p rag-core --test integration -- --ignored claude_live::"]
async fn claude_cli_answers_a_structured_question_live() {
    require_prod_api();
    let claude = ClaudeCli::new("haiku");

    let answer = claude
        .ask(Question {
            system_prompt: "Answer with the capital city of the country in the user message.",
            schema: r#"{"type":"object","properties":{"capital":{"type":"string"}},"required":["capital"],"additionalProperties":false}"#,
            input: "France",
        })
        .await
        .unwrap();

    assert_eq!(answer["capital"], "Paris", "the whole answer: {answer}");
}
