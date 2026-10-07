//! Runs a picture that stands alone through conversion and ingestion into a throwaway collection
//! of the local Qdrant and a throwaway graph of the local FalkorDB, with stand-ins for the paid
//! calls, so nothing is billed.

use std::path::{Path, PathBuf};

use graph::testing::{GraphSize, size, stored_concept_graph, stored_document};
use ocr::convert::convert_image_with;
use ocr::{ConvertedImage, read_chapter};
use rag_core::DocId;
use rag_ingestion::{Item, LoneImage, chapter_items, image_items, ingest_chapter, ingest_image};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::support::{
    self, CAPTION, LABEL, StandInImageServices, StandInLlm, ThrowawayStores, concept_points_in,
    points_in,
};

const NOTE: &str = "A chart from a book on option trading.";

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

/// What the model is asked about an item: where it sits, then the text that is embedded for it.
fn asked_about(item: &Item) -> String {
    format!("{}\n\n{}", item.input.title, item.input.text)
}

fn items_of(chapter_folder: &Path) -> Vec<Item> {
    chapter_items(&read_chapter(chapter_folder).unwrap())
}

fn picture_item(image: &ConvertedImage, note: Option<&str>) -> Item {
    image_items(&LoneImage { image, note }).remove(0)
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
    ingest_image(&lone, &models, &stores).await.unwrap();

    assert_eq!(services.calls(), 1, "one transcription call");
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

    // A second ingest of the same picture converts nothing and asks nothing.
    let after_first = (
        size(&stores.graph).await,
        stored_concept_graph(&stores.graph).await,
        points,
        concept_points_in(config).await,
    );
    assert_eq!(after_first.0, GraphSize { nodes: 3, edges: 2 });
    assert_eq!(after_first.3.len(), 1);
    let again = convert(&throwaway, &services).await;
    assert_eq!(again, image);
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
    let after_second = (
        size(&stores.graph).await,
        stored_concept_graph(&stores.graph).await,
        points_in(config).await,
        concept_points_in(config).await,
    );
    assert_eq!(after_second, after_first);
}

/// The passages that follow the heading of the related material, as the stored texts of the items
/// they were taken from. Each one starts with a line of its number and the title of its document.
fn related_passages(tail: &str, document_title: &str, stored_texts: &[String]) -> Vec<String> {
    let mut passages = Vec::new();
    let mut rest = tail;
    while !rest.is_empty() {
        let header = format!("{}. {document_title}, ", passages.len() + 1);
        let after_header = rest
            .strip_prefix(&header)
            .unwrap_or_else(|| panic!("the passage does not start with {header:?}: {rest}"));
        let (_, after_line) = after_header
            .split_once('\n')
            .expect("a header line ends with a line break");
        let text = stored_texts
            .iter()
            .filter(|text| after_line.starts_with(text.as_str()))
            .max_by_key(|text| text.len())
            .unwrap_or_else(|| panic!("no stored text starts the passage: {after_line}"));
        rest = after_line[text.len()..].strip_prefix("\n\n").unwrap_or("");
        passages.push(text.clone());
    }
    passages
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored image::"]
async fn an_item_alone_in_its_document_is_asked_about_with_possibly_related_material() {
    let throwaway = ThrowawayStores::new("image-related");
    let stores = throwaway.connect().await;
    let services = StandInImageServices::default();
    let model = StandInLlm::finding_nothing();
    let models = throwaway.models(model.clone());
    let chapter = support::intuition_chapter();
    let chapter_items = items_of(&chapter);
    let chapter_title = chapter_items[0].payload.doc_title.clone();
    let stored_texts: Vec<String> = chapter_items
        .iter()
        .map(|item| item.payload.text.clone())
        .collect();

    ingest_chapter(&chapter, &models, &stores).await.unwrap();
    let image = convert(&throwaway, &services).await;
    let lone = LoneImage {
        image: &image,
        note: None,
    };
    ingest_image(&lone, &models, &stores).await.unwrap();

    let questions = model.questions();
    assert_eq!(questions.len(), chapter_items.len() + 1);
    let heading = "Possibly related material";
    for question in &questions[..chapter_items.len()] {
        assert!(
            !question.input.contains(heading),
            "an item with neighbours is asked about alone: {}",
            question.input
        );
    }
    let question = &questions[chapter_items.len()];
    let own = asked_about(&picture_item(&image, None));
    let tail = question
        .input
        .strip_prefix(&own)
        .expect("the question starts with the text of the item")
        .strip_prefix(&format!("\n\n{heading}\n\n"))
        .expect("the heading follows the text of the item");
    let passages = related_passages(tail, &chapter_title, &stored_texts);
    assert_eq!(passages.len(), 5, "the five nearest items of the chapter");
    let prompt_file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ingest/concepts/prompts/extract.md");
    let prompt = std::fs::read_to_string(prompt_file).unwrap();
    assert_eq!(question.system_prompt, prompt);
    for words in [heading, "vocabulary", "not part of the item"] {
        assert!(prompt.contains(words), "the prompt does not say {words:?}");
    }

    // Another chapter is stored, so the nearest items of the picture are not the same ones. Its
    // answer was kept all the same, because the key leaves the related material out.
    let in_depth = support::in_depth_chapter();
    ingest_chapter(&in_depth, &models, &stores).await.unwrap();
    let asked_before = model.calls();
    let again: PathBuf = convert(&throwaway, &services).await.picture;
    assert_eq!(again, image.picture);
    let second = ingest_image(&lone, &models, &stores).await.unwrap();
    assert_eq!(
        model.calls(),
        asked_before,
        "the picture is not asked about again"
    );
    assert_eq!(second.concepts.llm_calls, 0);
    assert_eq!(second.concepts.cache_hits, 1);
    assert_eq!(services.calls(), 1);
}
