//! Runs the delete of one document, through `rag-ingest delete-document` and through the library,
//! against throwaway stores and a throwaway content folder, to show that it removes that document
//! from the stores and from disk and leaves the others alone. Concepts belong to no document, so
//! they stay.

use graph::GraphStore;
use graph::testing::{GraphSize, size, stored_concept_graph, stored_document};
use ocr::convert::convert_image_with;
use ocr::try_lock_chapter;
use rag_ingestion::{
    DeleteError, DocumentDeleteSummary, LoneImage, delete_document, ingest_chapter, ingest_image,
};
use serde_json::json;

use super::{
    copy_sample_chapter, give_an_upload_copy, ids_of, items_of, point_ids, printed_lines,
    run_delete_document, stderr_text, stdout_lines, write_upload,
};
use crate::support::{
    self, StandInImageServices, StandInLlm, ThrowawayStores, assert_document_stored,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn delete_document_removes_one_document_from_both_stores_and_leaves_the_other_whole() {
    let throwaway = ThrowawayStores::new("delete");
    let content = throwaway.config().content_folder.clone();
    let collection = throwaway.config().items_collection.clone();
    let stores = throwaway.connect().await;
    // The chapters are copied into the content folder of the throwaway stores: the delete removes
    // folders, and it must never reach the samples. A gets an upload copy that is its PDF, which
    // changes its id, so this comes before the ingest. B gets a file of the same kind that is not
    // its PDF.
    let chapter_a = copy_sample_chapter(&support::intuition_chapter(), &content);
    let chapter_b = copy_sample_chapter(&support::in_depth_chapter(), &content);
    let upload_a = give_an_upload_copy(&chapter_a, &content);
    let upload_b = write_upload(&chapter_b, &content, b"not the pdf of chapter 2");
    let items_a = items_of(&chapter_a);
    let items_b = items_of(&chapter_b);
    let (a, b) = (items_a.len() as u64, items_b.len() as u64);
    // Every item names the model that both chapters are about and one concept of its own chapter,
    // and says that the second is used for the first.
    let title_of_a = items_a[0].payload.doc_title.clone();
    let models = throwaway.models(StandInLlm::replying(move |input, _| {
        let own = if input.starts_with(&title_of_a) {
            "intuition about options"
        } else {
            "derivation of the model"
        };
        Ok(json!({
            "concepts": [
                { "name": "Black–Scholes model", "definition": "a model of option prices" },
                { "name": own, "definition": "a part of one chapter" },
            ],
            "relations": [{ "from": own, "type": "USED_FOR", "to": "Black–Scholes model" }],
        }))
    }));
    // What the graph holds when only the document of B, its items, its media, and the concepts
    // with their relations are left: the concepts and the two relations are three nodes and two
    // edges, and the media of both chapters is one more node.
    let size_with_b_and_concepts = GraphSize {
        nodes: b + 1 + 3 + 1,
        edges: b + (b - 1) + 2 * b + 2,
    };
    let first = ingest_chapter(support::chapter_at(&chapter_a), &models, &stores)
        .await
        .unwrap();
    let second = ingest_chapter(support::chapter_at(&chapter_b), &models, &stores)
        .await
        .unwrap();
    assert_ne!(first.doc_id, second.doc_id);
    assert_eq!(second.points_in_collection, a + b);
    let before = stored_concept_graph(&stores.graph).await;
    assert_eq!(before.concepts.len(), 3);
    assert_eq!(before.mentions.len() as u64, 2 * a + 2 * b);
    assert_eq!(before.relations.len(), 2);

    let output = run_delete_document(&throwaway, &first.doc_id.to_string());
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert_eq!(
        stdout_lines(&output),
        printed_lines(&DocumentDeleteSummary {
            doc_id: first.doc_id,
            collection: collection.clone(),
            points_removed: a,
            nodes_removed: a + 1,
            folders_removed: vec![chapter_a.clone()],
            uploads_removed: vec![upload_a.clone()],
        })
    );
    assert!(!chapter_a.exists(), "the converted folder of A goes");
    assert!(!upload_a.exists(), "the upload copy of A goes");
    assert!(chapter_b.is_dir(), "the converted folder of B stays");
    assert!(upload_b.is_file(), "the file under the uploads of B stays");
    assert!(stored_document(&stores.graph, first.doc_id).await.is_none());
    assert_document_stored(&stores.graph, &items_b).await;
    assert_eq!(size(&stores.graph).await, size_with_b_and_concepts);
    assert_eq!(point_ids(&throwaway).await, ids_of(&items_b));
    let after = stored_concept_graph(&stores.graph).await;
    assert_eq!(
        after.concepts, before.concepts,
        "the concepts stay, with their ids"
    );
    let ids_of_b = ids_of(&items_b);
    let mentions_of_b: Vec<_> = before
        .mentions
        .iter()
        .filter(|mention| ids_of_b.contains(&mention.item))
        .cloned()
        .collect();
    assert_eq!(mentions_of_b.len() as u64, 2 * b);
    assert_eq!(
        after.mentions, mentions_of_b,
        "only the mentions of the deleted document go"
    );
    assert_eq!(after.relations, before.relations, "the relations stay");

    // A delete that stopped half way, after the points and before the nodes, is finished by
    // running it again.
    let removed = stores.items.delete_document(second.doc_id).await.unwrap();
    assert_eq!(removed, b);
    assert_eq!(size(&stores.graph).await, size_with_b_and_concepts);
    let output = run_delete_document(&throwaway, &second.doc_id.to_string());
    assert!(output.status.success(), "{}", stderr_text(&output));
    assert_eq!(
        stdout_lines(&output),
        printed_lines(&DocumentDeleteSummary {
            doc_id: second.doc_id,
            collection,
            points_removed: 0,
            nodes_removed: b + 1,
            folders_removed: vec![chapter_b.clone()],
            uploads_removed: Vec::new(),
        })
    );
    assert!(!chapter_b.exists(), "the converted folder of B goes");
    assert!(
        upload_b.is_file(),
        "a file with other bytes than the PDF stays"
    );
    assert!(
        chapter_b.parent().unwrap().is_dir(),
        "a delete of a document never removes the folder of the media"
    );
    assert!(
        stored_document(&stores.graph, second.doc_id)
            .await
            .is_none()
    );
    assert_eq!(
        size(&stores.graph).await,
        GraphSize { nodes: 4, edges: 2 },
        "the concepts, their relations and the media are left"
    );
    let left = stored_concept_graph(&stores.graph).await;
    assert_eq!(left.concepts, before.concepts);
    assert!(left.mentions.is_empty());
    assert_eq!(left.relations, before.relations);
    assert!(point_ids(&throwaway).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn a_second_delete_removes_a_folder_that_is_all_that_is_left_and_a_third_is_refused() {
    let throwaway = ThrowawayStores::new("delete-folder-left");
    let content = throwaway.config().content_folder.clone();
    let collection = throwaway.config().items_collection.clone();
    let stores = throwaway.connect().await;
    let chapter = copy_sample_chapter(&support::intuition_chapter(), &content);
    let models = throwaway.models(StandInLlm::finding_nothing());
    let ingested = ingest_chapter(support::chapter_at(&chapter), &models, &stores)
        .await
        .unwrap();
    // A delete that stopped after the points and the nodes left only the folder.
    stores.items.delete_document(ingested.doc_id).await.unwrap();
    stores.graph.delete_document(ingested.doc_id).await.unwrap();

    let summary = delete_document(ingested.doc_id, &content, &stores)
        .await
        .unwrap();
    assert_eq!(
        summary,
        DocumentDeleteSummary {
            doc_id: ingested.doc_id,
            collection,
            points_removed: 0,
            nodes_removed: 0,
            folders_removed: vec![chapter.clone()],
            uploads_removed: Vec::new(),
        }
    );
    assert!(!chapter.exists());

    let refused = delete_document(ingested.doc_id, &content, &stores)
        .await
        .expect_err("nothing of the document is left");
    assert!(
        matches!(refused, DeleteError::UnknownDocument { .. }),
        "{refused:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn a_document_that_is_being_converted_is_not_deleted() {
    let throwaway = ThrowawayStores::new("delete-converting");
    let content = throwaway.config().content_folder.clone();
    let stores = throwaway.connect().await;
    let chapter = copy_sample_chapter(&support::intuition_chapter(), &content);
    let items = items_of(&chapter);
    let models = throwaway.models(StandInLlm::finding_nothing());
    let ingested = ingest_chapter(support::chapter_at(&chapter), &models, &stores)
        .await
        .unwrap();

    // A run that converts the chapter holds this lock, and a second lock is refused.
    let conversion = try_lock_chapter(&chapter)
        .unwrap()
        .expect("no run holds the chapter");
    let refused = delete_document(ingested.doc_id, &content, &stores)
        .await
        .expect_err("the chapter is being converted");
    assert!(
        matches!(&refused, DeleteError::Converting { folder } if folder == &chapter),
        "{refused:?}"
    );
    assert_document_stored(&stores.graph, &items).await;
    assert_eq!(point_ids(&throwaway).await, ids_of(&items));
    assert!(chapter.is_dir());

    drop(conversion);
    let summary = delete_document(ingested.doc_id, &content, &stores)
        .await
        .unwrap();
    assert_eq!(summary.folders_removed, vec![chapter.clone()]);
    assert!(!chapter.exists());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn a_picture_that_stands_alone_is_deleted_with_its_folder() {
    let throwaway = ThrowawayStores::new("delete-picture");
    let content = throwaway.config().content_folder.clone();
    let collection = throwaway.config().items_collection.clone();
    let stores = throwaway.connect().await;
    let image = convert_image_with(
        &support::sample_picture(),
        &content,
        &StandInImageServices::default(),
    )
    .await
    .unwrap();
    let models = throwaway.models(StandInLlm::finding_nothing());
    let lone = LoneImage {
        image: &image,
        note: None,
    };
    let ingested = ingest_image(&lone, &models, &stores).await.unwrap();
    let folder = content
        .join("images")
        .join(&image.index.source_sha256[..16]);
    assert!(folder.join("image.json").is_file());

    let summary = delete_document(ingested.doc_id, &content, &stores)
        .await
        .unwrap();
    assert_eq!(
        summary,
        DocumentDeleteSummary {
            doc_id: ingested.doc_id,
            collection,
            points_removed: 1,
            nodes_removed: 2,
            folders_removed: vec![folder.clone()],
            uploads_removed: Vec::new(),
        }
    );
    assert!(!folder.exists());
    assert!(
        stored_document(&stores.graph, ingested.doc_id)
            .await
            .is_none()
    );
    assert!(point_ids(&throwaway).await.is_empty());
}
