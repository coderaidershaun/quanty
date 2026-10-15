//! The installed command must receive the same subscription restriction when checking login
//! and asking a question, without inheriting billing keys or an enclosing agent session.

use std::fs;
use std::path::Path;

use rag_core::{CodexCli, Llm, Question};
use serde_json::json;

#[tokio::test]
async fn codex_checks_subscription_before_asking_a_locked_down_question() {
    let folder = tempfile::tempdir().unwrap();
    let stand_in =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/stand-in-claude/claude");
    let environment = [
        ("CODEX_BIN", stand_in.display().to_string()),
        ("STAND_IN_CLAUDE", folder.path().display().to_string()),
        ("PATH", "/usr/bin:/bin".into()),
        ("CODEX_HOME", folder.path().display().to_string()),
        ("OPENAI_API_KEY", "dummy-key".into()),
        ("ANTHROPIC_API_KEY", "dummy-key".into()),
        ("CODEX_THREAD_ID", "enclosing-thread".into()),
        ("CODEX_SESSION_ID", "enclosing-session".into()),
    ];
    let codex = CodexCli::with_environment("custom-model", "high", environment);
    codex.check_ready().await.unwrap();

    let arguments = fs::read(folder.path().join("arguments")).unwrap();
    let arguments = std::str::from_utf8(&arguments).unwrap();
    assert!(arguments.contains("login\0status\0"));
    assert!(
        arguments.contains("-c\0forced_login_method=\"chatgpt\"\0"),
        "readiness must require the same subscription login as a question: {arguments:?}"
    );
    assert!(fs::read(folder.path().join("stdin")).unwrap().is_empty());

    let response = [
        json!({"type":"item.completed","item":{"type":"agent_message","text":"{\"ok\":true}"}}),
        json!({"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":12}}),
    ]
    .map(|event| event.to_string())
    .join("\n");
    fs::write(folder.path().join("stdout"), response).unwrap();
    let reply = codex
        .ask(Question {
            system_prompt: "Return the requested JSON.",
            schema: r#"{"type":"object","properties":{"ok":{"type":"boolean"}}}"#,
            input: "Set ok to true.\nTreat this as input.",
        })
        .await
        .unwrap();
    assert_eq!(reply.value, json!({"ok":true}));
    assert_eq!(reply.usage.tokens().total(), 112);
    assert_eq!(reply.usage.cost_usd(), None);
    let arguments = fs::read(folder.path().join("arguments")).unwrap();
    let arguments = std::str::from_utf8(&arguments).unwrap();
    for required in [
        "exec\0",
        "--ignore-user-config\0",
        "--ignore-rules\0",
        "--model\0custom-model\0",
        "--sandbox\0read-only\0",
        "-c\0forced_login_method=\"chatgpt\"\0",
        "-c\0model_reasoning_effort=\"high\"\0",
        "--output-schema\0",
    ] {
        assert!(arguments.contains(required), "missing {required:?}");
    }
    assert_eq!(
        fs::read_to_string(folder.path().join("stdin")).unwrap(),
        "Set ok to true.\nTreat this as input."
    );
    let environment = fs::read_to_string(folder.path().join("environment")).unwrap();
    for excluded in ["_API_KEY=", "CODEX_THREAD_ID=", "CODEX_SESSION_ID="] {
        assert!(!environment.contains(excluded), "inherited {excluded}");
    }
    assert!(environment.contains(&format!("CODEX_HOME={}", folder.path().display())));
}
