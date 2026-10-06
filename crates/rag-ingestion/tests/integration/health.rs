//! Runs `rag-ingest health` as a person would, once against ports where nothing listens and once
//! against the real local stores.

use std::process::{Command, Output};

const LOCAL_RUN_COMMAND: &str =
    "cargo test -p rag-ingestion --test integration -- --ignored health::";

fn run_health(configure: impl FnOnce(&mut Command)) -> (Output, Vec<String>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rag-ingest"));
    command.arg("health");
    configure(&mut command);
    let output = command
        .output()
        .expect("the rag-ingest binary should start");
    let lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    (output, lines)
}

#[test]
fn health_names_each_store_it_cannot_reach_and_exits_non_zero() {
    // The Qdrant address comes from a `.env` one folder above the folder the command runs in,
    // and the FalkorDB address from the environment, which wins over that file. Both name a
    // closed port on this machine, which refuses at once, and the real settings stay out.
    let settings_folder = tempfile::tempdir().unwrap();
    std::fs::write(
        settings_folder.path().join(".env"),
        "QDRANT_URL=http://127.0.0.1:1\nFALKORDB_URL=falkor://127.0.0.1:2\n",
    )
    .unwrap();
    let working_folder = settings_folder.path().join("nested");
    std::fs::create_dir(&working_folder).unwrap();
    let (output, lines) = run_health(|command| {
        command
            .current_dir(&working_folder)
            .env_remove("QDRANT_URL")
            .env("FALKORDB_URL", "falkor://127.0.0.1:1");
    });

    assert_eq!(output.status.code(), Some(1), "{lines:?}");
    assert_eq!(lines.len(), 3, "one line for each check: {lines:?}");
    assert!(
        lines[0].starts_with("Qdrant: FAILED (http://127.0.0.1:1)"),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("FalkorDB: FAILED (falkor://127.0.0.1:1)"),
        "{}",
        lines[1]
    );
    assert!(lines[2].starts_with("claude:"), "{}", lines[2]);
}

#[test]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and a signed-in claude CLI, and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored health::"]
fn health_reports_both_stores_and_the_claude_sign_in() {
    let (output, lines) = run_health(|_| {});

    assert_eq!(
        lines.len(),
        3,
        "one line for each check: {lines:?}; start the stores and sign in to claude, then run {LOCAL_RUN_COMMAND}"
    );
    assert!(lines[0].starts_with("Qdrant: ok ("), "{}", lines[0]);
    assert!(lines[1].starts_with("FalkorDB: ok ("), "{}", lines[1]);
    assert_eq!(lines[2], "claude: ok (signed in)");
    assert!(output.status.success());
}
