//! Runs a picture that stands alone through conversion and ingestion into throwaway stores, with
//! stand-ins for the paid calls, so nothing is billed.

use std::path::Path;

use graph::testing::{GraphSize, size, stored_concept_graph, stored_document};
use ocr::convert::convert_image_with;
use ocr::convert::reply::TranscribedPage;
use ocr::convert::services::{Answer, ClaudeError, ImageServices, ServiceError};
use ocr::{ConvertError, ConvertedImage};
use rag_core::{DocId, Usage};
use rag_ingestion::{IngestSummary, LoneImage, ingest_image};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::support::{
    self, CAPTION, LABEL, StandInImageServices, StandInLlm, ThrowawayStores, concept_points_in,
    points_in,
};

const NOTE: &str = "A chart from a book on option trading.";

fn picture_reading(summary: &IngestSummary) -> Option<Usage> {
    summary
        .usage
        .models()
        .find(|(model, _)| *model == "stand-in-picture-reader")
        .map(|(_, used)| used.tokens)
}

fn sha256_hex(path: &Path) -> String {
    Sha256::digest(std::fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn convert(throwaway: &ThrowawayStores, services: &StandInImageServices) -> ConvertedImage {
    convert_image_with(
        &support::sample_picture(),
        &throwaway.config().content_folder,
        services,
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored image::"]
async fn a_lone_picture_becomes_a_one_figure_document_and_a_second_ingest_calls_nothing() {
    let throwaway = ThrowawayStores::new("image");
    let config = throwaway.config();
    let stores = throwaway.connect().await;
    let services = StandInImageServices::default();
    let model = StandInLlm::replying(|_, _| {
        Ok(json!({
            "concepts": [{ "name": "implied volatility", "definition": "the volatility that an option price implies" }],
            "relations": [],
        }))
    });
    let models = throwaway.models(model.clone());
    let sha256 = sha256_hex(&support::sample_picture());

    let image = convert(&throwaway, &services).await;
    let lone = LoneImage {
        image: &image,
        note: Some(NOTE),
    };
    let first = ingest_image(&lone, &models, &stores).await.unwrap();

    assert_eq!(services.calls(), 1, "one transcription call");
    let one_transcription = Usage {
        input_tokens: 100,
        output_tokens: 1,
        ..Usage::default()
    };
    assert_eq!(picture_reading(&first), Some(one_transcription));
    let folder = config.content_folder.join("images").join(&sha256[..16]);
    for file in ["figure.md", "image.json", "picture.png"] {
        assert!(folder.join(file).is_file(), "{file} is not in {folder:?}");
    }
    assert_eq!(
        std::fs::read(folder.join("picture.png")).unwrap(),
        std::fs::read(support::sample_picture()).unwrap(),
        "the copy has the bytes of the sample"
    );
    let canonical_content_folder = std::fs::canonicalize(&config.content_folder).unwrap();
    assert!(
        image.picture.starts_with(&canonical_content_folder),
        "{:?} is not under {canonical_content_folder:?}",
        image.picture
    );
    assert_eq!(image.index.source_sha256, sha256);
    assert_eq!(image.index.label.as_deref(), Some(LABEL));
    assert_eq!(image.index.caption.as_deref(), Some(CAPTION));

    let inputs = models.embedder.received();
    let item_inputs: Vec<_> = inputs
        .iter()
        .filter(|input| input.image.is_some())
        .collect();
    assert_eq!(item_inputs.len(), 1, "one input has a picture");
    let input = item_inputs[0];
    assert_eq!(input.image.as_ref(), Some(&image.picture));
    assert_eq!(input.title, "volatility-surface.png");
    let explained_at = input
        .text
        .find(&image.explanation)
        .expect("the explanation");
    let noted_at = input.text.find("Note: ").expect("the note");
    assert!(
        explained_at < noted_at,
        "the note comes after the explanation"
    );
    assert!(input.text.ends_with(&format!("Note: {NOTE}")));

    let questions = model.questions();
    assert_eq!(questions.len(), 1);
    assert!(questions[0].input.contains(&image.explanation));
    assert!(questions[0].input.ends_with(&format!("Note: {NOTE}")));

    let points = points_in(config).await;
    assert_eq!(points.len(), 1);
    let (id, payload) = &points[0];
    let doc_id = DocId::from_source_sha256(&sha256);
    assert_eq!(payload["doc_id"], doc_id.to_string());
    assert_eq!(payload["kind"], "figure");
    assert_eq!(payload["page"], 1);
    assert_eq!(payload["label"], LABEL);
    assert!(
        payload.get("media").is_none(),
        "a lone picture belongs to no media: {payload}"
    );
    assert_eq!(
        payload["image_path"],
        image.picture.to_str().expect("a UTF-8 path")
    );
    assert!(
        payload["text"].as_str().unwrap().ends_with(NOTE),
        "{payload}"
    );
    let stored = stored_document(&stores.graph, doc_id)
        .await
        .expect("the graph holds the document");
    assert_eq!(stored.title, "volatility-surface.png");
    assert_eq!(stored.items.len(), 1);
    assert_eq!(&stored.items[0].id, id);
    assert_eq!(stored.items[0].kind, "figure");
    assert_eq!(stored.items[0].page, 1);

    let after_first = (
        size(&stores.graph).await,
        stored_concept_graph(&stores.graph).await,
        points,
        concept_points_in(config).await,
    );
    assert_eq!(after_first.0, GraphSize { nodes: 3, edges: 2 });
    assert_eq!(after_first.3.len(), 1);
    let again = convert(&throwaway, &services).await;
    assert_eq!(
        (&again.index, &again.explanation, &again.picture),
        (&image.index, &image.explanation, &image.picture)
    );
    let second = ingest_image(
        &LoneImage {
            image: &again,
            note: Some(NOTE),
        },
        &models,
        &stores,
    )
    .await
    .unwrap();
    assert_eq!(services.calls(), 1, "no second transcription call");
    assert_eq!(model.calls(), 1, "no second question");
    assert_eq!(second.concepts.llm_calls, 0);
    assert_eq!(second.concepts.cache_hits, 1);
    assert_eq!(
        picture_reading(&second),
        None,
        "a saved picture is not paid for again"
    );
    let after_second = (
        size(&stores.graph).await,
        stored_concept_graph(&stores.graph).await,
        points_in(config).await,
        concept_points_in(config).await,
    );
    assert_eq!(after_second, after_first);
}

/// A model that does not answer in time, for every picture.
struct TimedOutImageServices;

impl ImageServices for TimedOutImageServices {
    async fn transcribe(
        &self,
        _picture: &Path,
        _correction: Option<&str>,
    ) -> Result<Answer<TranscribedPage>, ServiceError> {
        Err(ClaudeError::TimedOut { seconds: 1 }.into())
    }
}

#[tokio::test]
async fn a_picture_whose_paid_call_fails_is_named_with_what_happened_to_the_call() {
    let content_folder = tempfile::tempdir().unwrap();

    let error = convert_image_with(
        &support::sample_picture(),
        content_folder.path(),
        &TimedOutImageServices,
    )
    .await
    .unwrap_err();

    assert!(
        matches!(
            error,
            ConvertError::ImageCallFailed {
                source: ServiceError::Claude(ClaudeError::TimedOut { seconds: 1 }),
                ..
            }
        ),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("volatility-surface.png"), "{message}");
    assert!(
        !message.contains("could not be started"),
        "the services were started, and it is the call that failed: {message}"
    );
}
