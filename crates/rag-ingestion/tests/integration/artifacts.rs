//! Metadata checkpoints are real files, while the paid model follows a local script.

use ocr::content::ChapterArtifacts;
use rag_ingestion::{ArtifactWriter, testing::StandInLlm};
use serde_json::json;

fn chapter() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("chapter.json"), json!({
        "format-version":1,"media-title":"Context","name":{"title":"Context"},
        "source-file":"context.pdf","source-sha256":"ab".repeat(32),"page-count":1,"finished":true
    }).to_string()).unwrap();
    let page = folder.path().join("page-num-1");
    std::fs::create_dir(&page).unwrap();
    std::fs::write(page.join("page.json"), json!({"format-version":1,"page-position":1,
        "pieces":[{"number":1,"file":"formula.md","kind":"formula","statement":"Finds value","symbols":[{"symbol":"y","meaning":"later price"}]},
        {"number":2,"file":"text.md","kind":"text"}]}).to_string()).unwrap();
    std::fs::write(page.join("formula.md"), "x = y").unwrap();
    std::fs::write(
        page.join("text.md"),
        "The later definition says y is the price.",
    )
    .unwrap();
    folder
}

fn answer(question: &rag_ingestion::testing::AskedQuestion) -> serde_json::Value {
    rag_ingestion::testing::stand_in_artifacts_reply(&question.input).unwrap()
}

#[tokio::test]
async fn chapter_artifacts_persist_and_resume() {
    let folder = chapter();
    let page = folder.path().join("page-num-1");
    let mut file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(page.join("page.json")).unwrap()).unwrap();
    for (number, kind, fields) in [
        (3, "figure", json!({"caption":"Shown figure"})),
        (4, "table", json!({"summary":"Table summary"})),
        (
            5,
            "code",
            json!({"summary":"First listing part","language":"rust"}),
        ),
    ] {
        let mut entry = json!({"number":number,"file":format!("{kind}.txt"),"kind":kind});
        entry
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        file["pieces"].as_array_mut().unwrap().push(entry);
        std::fs::write(page.join(format!("{kind}.txt")), format!("Source {kind}")).unwrap();
    }
    std::fs::write(page.join("page.json"), file.to_string()).unwrap();
    let later = folder.path().join("page-num-2");
    std::fs::create_dir(&later).unwrap();
    std::fs::write(later.join("page.json"),json!({"format-version":1,"page-position":2,"pieces":[{"number":1,"kind":"code","file":"code.txt","summary":"Second listing part","language":"rust"}]}).to_string()).unwrap();
    std::fs::write(later.join("code.txt"), "Later listing definition").unwrap();
    let mut index = ocr::ChapterIndex::read(folder.path()).unwrap();
    index.page_count = 2;
    index.write(folder.path()).unwrap();
    let llm = StandInLlm::replying_to_questions(|question, _| {
        for context in [
            "later definition",
            "Later listing definition",
            "Table summary",
            "First listing part",
            "Finds value",
            "later price",
        ] {
            assert!(question.input.contains(context), "{context}");
        }
        let input: serde_json::Value = serde_json::from_str(&question.input).unwrap();
        assert_eq!(input["manifest"].as_array().unwrap().len(), 4);
        assert_eq!(input["manifest"][3]["pieces"].as_array().unwrap().len(), 2);
        let mut value = answer(question);
        value["artifacts"][3]["title"] = json!("Prefetching with __builtin_prefetch");
        value["artifacts"][3]["description"] =
            json!("Warms std::vector<InstrumentState> for the next event.");
        Ok(value)
    });
    let writer = ArtifactWriter::new(llm.clone());
    let first = writer.write_missing(folder.path()).await.unwrap();
    assert_eq!(first.total, 4);
    let mut saved = ChapterArtifacts::read(folder.path()).unwrap().unwrap();
    assert_eq!(saved.entries.len(), 4);
    assert!(saved.matches_source(&ocr::read_chapter(folder.path()).unwrap().artifact_source()));
    assert_eq!(
        saved.entries[3].text.title,
        "Prefetching with __builtin_prefetch"
    );
    let mut ambiguous_saved = saved.clone();
    ambiguous_saved
        .source
        .manifest
        .push(ambiguous_saved.source.manifest[0].clone());
    assert!(!ambiguous_saved.matches_source(&ambiguous_saved.source));
    assert!(
        saved
            .entries
            .iter()
            .all(|entry| entry.model == "stand-in" && !entry.cache_identity.is_empty())
    );
    writer.write_missing(folder.path()).await.unwrap();
    assert_eq!(llm.calls(), 1);
    saved.entries.truncate(2);
    saved.write(folder.path()).unwrap();
    let other = StandInLlm::replying_to_questions(|q, _| Ok(answer(q))).named("other-model");
    ArtifactWriter::new(other.clone())
        .write_missing(folder.path())
        .await
        .unwrap();
    let resumed = ChapterArtifacts::read(folder.path()).unwrap().unwrap();
    assert_eq!(resumed.entries.len(), 4);
    assert_eq!(other.calls(), 1);
    assert_eq!(resumed.entries[0].model, "stand-in");
    assert_eq!(resumed.entries[2].model, "other-model");
    ArtifactWriter::new(llm.clone().named("third-model"))
        .write_missing(folder.path())
        .await
        .unwrap();
    assert_eq!(llm.calls(), 1);
    let mut old = resumed;
    old.generation_signature = "old policy".to_owned();
    old.write(folder.path()).unwrap();
    assert!(old.matches_source(&ocr::read_chapter(folder.path()).unwrap().artifact_source()));
    writer.write_missing(folder.path()).await.unwrap();
    assert_eq!(llm.calls(), 2);
    std::fs::write(page.join("text.md"), "A changed later definition").unwrap();
    writer.write_missing(folder.path()).await.unwrap();
    assert_eq!(llm.calls(), 3);

    let many = chapter();
    let page = many.path().join("page-num-1");
    let pieces: Vec<_> = (1..=21).map(|number| json!({"number":number,"kind":"formula","file":"formula.md","statement":"Value"})).collect();
    std::fs::write(
        page.join("page.json"),
        json!({"format-version":1,"page-position":1,"pieces":pieces}).to_string(),
    )
    .unwrap();
    let partial = StandInLlm::replying_to_questions(|q, _| {
        let input: serde_json::Value = serde_json::from_str(&q.input).unwrap();
        if input["targets"].as_array().unwrap().len() == 1 {
            Err(rag_core::LlmError::UsageLimit {
                message: "stop".to_owned(),
            })
        } else {
            Ok(answer(q))
        }
    });
    assert!(
        ArtifactWriter::new(partial)
            .write_missing(many.path())
            .await
            .is_err()
    );
    assert_eq!(
        ChapterArtifacts::read(many.path())
            .unwrap()
            .unwrap()
            .entries
            .len(),
        20
    );
    let finishing = StandInLlm::replying_to_questions(|q, _| Ok(answer(q)));
    let summary = ArtifactWriter::new(finishing.clone())
        .write_missing(many.path())
        .await
        .unwrap();
    assert_eq!(summary.written, 1);
    assert_eq!(finishing.calls(), 1);
    assert!(!many.path().join("artifacts.json.tmp").exists());
}

#[tokio::test]
async fn artifact_requests_above_the_old_budget_can_finish() {
    use rag_ingestion::{ArtifactState, assess_artifacts};

    let folder = chapter();
    std::fs::write(
        folder.path().join("page-num-1/text.md"),
        "é".repeat(450_000),
    )
    .unwrap();
    assert_eq!(
        assess_artifacts(folder.path()).state,
        ArtifactState::Missing
    );
    let llm = StandInLlm::replying_to_questions(|question, _| {
        let bytes = question.input.len() + question.schema.len() + question.system_prompt.len();
        assert!(bytes > 100_000 && bytes <= 1_000_000, "{bytes}");
        Ok(answer(question))
    });
    let summary = ArtifactWriter::new(llm.clone())
        .write_missing(folder.path())
        .await
        .unwrap();
    assert_eq!(summary.written, 1);
    assert_eq!(llm.calls(), 1);
    assert_eq!(assess_artifacts(folder.path()).state, ArtifactState::Ready);
}

#[tokio::test]
async fn invalid_or_oversized_artifact_requests_cannot_finish() {
    use rag_ingestion::{ArtifactError, ArtifactState, assess_artifacts};
    let folder = chapter();
    let mutations = crate::support::artifacts::REJECTIONS;
    let mut formatting_rejection = None;
    for &(mutation, reason) in mutations {
        let llm = StandInLlm::replying_to_questions(move |q, _| {
            let mut value = answer(q);
            mutation(&mut value);
            Ok(value)
        });
        let Err(ArtifactError::Rejected { diagnostics }) = ArtifactWriter::new(llm.clone())
            .write_missing(folder.path())
            .await
        else {
            panic!("expected rejection: {reason}");
        };
        assert!(diagnostics.contains(reason), "{diagnostics}");
        if reason == "title: Markdown formatting" {
            formatting_rejection = Some((mutation, diagnostics.clone()));
        }
        if reason.contains("control character") || reason.contains("10000") {
            assert!(diagnostics.contains("\\0"), "{diagnostics}");
            assert!(
                diagnostics.len() < 400,
                "bounded escaped excerpt: {}",
                diagnostics.len()
            );
            assert!(!diagnostics.chars().any(char::is_control));
        }
        assert_eq!(llm.calls(), 2);
        assert!(!folder.path().join("artifacts.json").exists());
        let second: serde_json::Value = serde_json::from_str(&llm.questions()[1].input).unwrap();
        assert_eq!(second["validation_feedback"], diagnostics);
    }
    let retry_too_large = StandInLlm::replying_to_questions(|question, _| {
        assert!(
            question.input.len() + question.schema.len() + question.system_prompt.len()
                <= 1_000_000
        );
        let mut value = answer(question);
        value["artifacts"][0]["id"] = json!("unknown".repeat(200_000));
        Ok(value)
    });
    let mut reported = rag_core::UsageTally::default();
    let error = ArtifactWriter::new(retry_too_large.clone())
        .write_missing_with_progress(folder.path(), &mut |_, spent| reported = spent.clone())
        .await
        .unwrap_err();
    assert!(matches!(error, ArtifactError::TooLarge { .. }));
    assert_eq!(retry_too_large.calls(), 1);
    assert!(
        !reported.is_empty(),
        "rejected reply spend is reported before budget failure"
    );
    assert!(!folder.path().join("artifacts.json").exists());
    let (formatting_mutation, formatting_feedback) = formatting_rejection.unwrap();
    let repaired = StandInLlm::replying_to_questions(move |q, _| {
        let input: serde_json::Value = serde_json::from_str(&q.input).unwrap();
        let mut value = answer(q);
        if input["validation_feedback"] == "" {
            formatting_mutation(&mut value);
        } else {
            assert_eq!(input["validation_feedback"], formatting_feedback);
        }
        Ok(value)
    });
    ArtifactWriter::new(repaired.clone())
        .write_missing(folder.path())
        .await
        .unwrap();
    assert_eq!(repaired.calls(), 2);
    std::fs::remove_file(folder.path().join("artifacts.json")).unwrap();
    let systemic = StandInLlm::replying_to_questions(|_, _| {
        Err(rag_core::LlmError::UsageLimit {
            message: "limit".to_owned(),
        })
    });
    let error = ArtifactWriter::new(systemic.clone())
        .write_missing(folder.path())
        .await
        .unwrap_err();
    assert!(error.stops_the_run());
    assert_eq!(systemic.calls(), 1);
    std::fs::write(
        folder.path().join("page-num-1/text.md"),
        "é".repeat(600_000),
    )
    .unwrap();
    let llm = StandInLlm::replying_to_questions(|q, _| Ok(answer(q)));
    assert_eq!(
        assess_artifacts(folder.path()).state,
        ArtifactState::TooLarge
    );
    assert!(matches!(
        ArtifactWriter::new(llm.clone())
            .write_missing(folder.path())
            .await,
        Err(ArtifactError::TooLarge { .. })
    ));
    assert_eq!(llm.calls(), 0);
    let no_targets = chapter();
    std::fs::write(no_targets.path().join("page-num-1/page.json"),json!({"format-version":1,"page-position":1,"pieces":[{"number":2,"kind":"text","file":"text.md"}]}).to_string()).unwrap();
    std::fs::write(
        no_targets.path().join("page-num-1/text.md"),
        "x".repeat(1_200_000),
    )
    .unwrap();
    assert_eq!(
        ArtifactWriter::new(llm.clone())
            .write_missing(no_targets.path())
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(llm.calls(), 0);
    for (title, description) in [
        (
            "Software prefetching with __builtin_prefetch",
            "Prefetches std::vector<InstrumentState> before the MarketDataHandler loop.",
        ),
        (
            "__VA_ARGS__",
            "Expands a variadic macro with std::vector<std::vector<int>>.",
        ),
        (
            "#include <vector>",
            "Includes <vector> for int **p, *ptr and a[i](x).",
        ),
        (
            "*ptr",
            "Compares a $100 strike with prices < 120 and payoffs > 0.",
        ),
    ]
    .into_iter()
    .map(|(title, description)| (title.to_owned(), description.to_owned()))
    .chain(std::iter::once(("é".repeat(80), "界".repeat(280))))
    {
        let technical = chapter();
        let plain = StandInLlm::replying_to_questions(move |q, _| {
            let mut value = answer(q);
            value["artifacts"][0]["title"] = json!(title);
            value["artifacts"][0]["description"] = json!(description);
            Ok(value)
        });
        ArtifactWriter::new(plain)
            .write_missing(technical.path())
            .await
            .unwrap();
    }
    let duplicate = chapter();
    let page = duplicate.path().join("page-num-1/page.json");
    let mut file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&page).unwrap()).unwrap();
    let first = file["pieces"][0].clone();
    file["pieces"].as_array_mut().unwrap().push(first);
    std::fs::write(page, file.to_string()).unwrap();
    let ambiguous = StandInLlm::replying_to_questions(|q, _| {
        let mut value = answer(q);
        value["artifacts"].as_array_mut().unwrap().truncate(1);
        Ok(value)
    });
    assert!(
        ArtifactWriter::new(ambiguous.clone())
            .write_missing(duplicate.path())
            .await
            .is_err(),
        "duplicate source identities cannot claim exact coverage"
    );
    assert_eq!(
        ambiguous.calls(),
        0,
        "ambiguous source must fail before paid work"
    );
    assert_ne!(
        assess_artifacts(duplicate.path()).state,
        ArtifactState::Ready
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs local FalkorDB and Qdrant; only throwaway stores and a subprocess stand-in model"]
async fn rag_ingest_artifacts_backfills_without_rebuilding_stores() {
    use crate::support::{self, ThrowawayStores};
    use rag_ingestion::{ChapterChoice, stored_chapters};
    let throwaway = ThrowawayStores::new("artifacts-cli");
    let stores = throwaway.connect().await;
    let folder = chapter();
    let content =
        crate::delete::copy_sample_chapter(folder.path(), &throwaway.config().content_folder);
    let models = throwaway.models(StandInLlm::finding_nothing());
    let summary = rag_ingestion::ingest_chapter(
        rag_ingestion::testing::chapter_at(&content),
        &models,
        &stores,
    )
    .await
    .unwrap();
    std::fs::remove_file(content.join("artifacts.json")).unwrap();
    let points = support::points_in(throwaway.config()).await;
    let graph = graph::testing::size(&stores.graph).await;
    let run = |arguments: Vec<String>| {
        let mut command = support::rag_ingest_command(&throwaway, arguments);
        let bin = throwaway.temporary_folder().join("artifact-bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("claude");
        std::fs::write(&script,r#"#!/usr/bin/env python3
import json,sys,os
with open(os.path.join(os.environ['STAND_IN_CLAUDE'],'calls'),'a') as file: file.write('artifact-run\n')
if sys.argv[1:3]==['auth','status']: sys.exit(0)
value=json.load(sys.stdin)
print(json.dumps({'type':'result','subtype':'success','is_error':False,'structured_output':{'artifacts':[{'id':target['id'],'title':'Price equality','description':'Equates the source prices.'} for target in value['targets']]},'total_cost_usd':0}))
"#).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        command
            .current_dir(throwaway.temporary_folder())
            .env(
                "PATH",
                format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
            )
            .env("LLM_PROVIDER", "claude")
            .env("LLM_MODEL", "sonnet")
            .env("ANTHROPIC_API_KEY", "")
            .env("EMBEDDING_GEMINI_API_KEY", "")
            .env("QDRANT_URL", "http://127.0.0.1:1");
        command.output().unwrap()
    };
    let preview = run(vec!["artifacts".into(), "--all".into(), "--dry-run".into()]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(String::from_utf8_lossy(&preview.stdout).contains("missing"));
    assert_eq!(support::stand_in_claude_runs(&throwaway), 0);
    let one = run(vec![
        "artifacts".into(),
        "--document".into(),
        summary.doc_id.to_string(),
    ]);
    assert!(
        one.status.success(),
        "{}",
        String::from_utf8_lossy(&one.stderr)
    );
    assert_eq!(
        ChapterArtifacts::read(&content).unwrap().unwrap().entries[0]
            .text
            .title,
        "Price equality"
    );
    let calls = support::stand_in_claude_runs(&throwaway);
    for selector in [
        vec!["--all".into()],
        vec!["--media".into(), "CONTEXT".into()],
    ] {
        let mut args = vec!["artifacts".into()];
        args.extend(selector);
        assert!(run(args).status.success());
    }
    assert_eq!(
        support::stand_in_claude_runs(&throwaway),
        calls,
        "cached runs ask nothing"
    );
    assert!(calls >= 1);
    let mut partial = ChapterArtifacts::read(&content).unwrap().unwrap();
    partial.entries.clear();
    partial.write(&content).unwrap();
    let preview = run(vec!["artifacts".into(), "--all".into(), "--dry-run".into()]);
    assert!(String::from_utf8_lossy(&preview.stdout).contains("partial"));
    assert_eq!(support::stand_in_claude_runs(&throwaway), calls);
    assert!(
        run(vec!["artifacts".into(), "--all".into()])
            .status
            .success()
    );
    assert_eq!(support::points_in(throwaway.config()).await, points);
    assert_eq!(graph::testing::size(&stores.graph).await, graph);
    assert_eq!(
        stored_chapters(
            ChapterChoice::All,
            &stores.graph,
            &throwaway.config().content_folder
        )
        .await
        .unwrap()
        .len(),
        1
    );
    std::fs::remove_dir_all(content).unwrap();
    assert!(
        !run(vec!["artifacts".into(), "--all".into()])
            .status
            .success()
    );
    assert!(
        !run(vec![
            "artifacts".into(),
            "--document".into(),
            rag_core::DocId::from_source_sha256("missing").to_string()
        ])
        .status
        .success()
    );
}
