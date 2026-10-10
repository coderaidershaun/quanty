//! Runs the delete of a whole media against throwaway stores and a throwaway content folder, to
//! show which documents and folders go with the title and which stay.

use std::fs;

use graph::testing::{GraphSize, size, stored_concept_graph, stored_document};
use graph::{GraphStore, MediaNode};
use ocr::ChapterIndex;
use rag_core::MediaLabels;
use rag_ingestion::{
    DeleteError, DocumentDeleteSummary, MediaDeleteSummary, delete_media, ingest_chapter,
};
use serde_json::json;

use super::{
    UPLOADS_FOLDER, copy_sample_chapter, give_an_upload_copy, ids_of, items_of, point_ids,
    run_delete_media, stderr_text, stdout_lines,
};
use crate::support::{
    self, StandInLlm, ThrowawayStores, assert_document_stored, concept_points_in,
    size_of_one_document,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn a_media_with_no_document_is_deleted_and_a_title_that_nothing_carries_is_refused() {
    let throwaway = ThrowawayStores::new("delete-empty-media");
    let content = throwaway.config().content_folder.clone();
    let collection = throwaway.config().items_collection.clone();
    let stores = throwaway.connect().await;
    stores
        .graph
        .add_media(&MediaNode {
            title: "Quanty Sample Notes".to_owned(),
            labels: MediaLabels::default(),
        })
        .await
        .unwrap();

    let asked = "  quanty SAMPLE notes ";
    let summary = delete_media(asked, &content, &stores).await.unwrap();
    assert_eq!(
        summary,
        MediaDeleteSummary {
            title: asked.to_owned(),
            collection,
            documents: Vec::new(),
            media_folders_removed: Vec::new(),
            media_node_removed: true,
        }
    );
    assert_eq!(stores.graph.media().await.unwrap(), vec![]);

    let refused = delete_media(asked, &content, &stores)
        .await
        .expect_err("nothing carries the title any more");
    assert!(
        matches!(&refused, DeleteError::UnknownMedia { title } if title == asked),
        "{refused:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn a_media_folder_that_still_holds_something_stays_and_documents_need_no_media_node() {
    let throwaway = ThrowawayStores::new("delete-shared-folder");
    let content = throwaway.config().content_folder.clone();
    let collection = throwaway.config().items_collection.clone();
    let stores = throwaway.connect().await;
    let chapter_1 = copy_sample_chapter(&support::intuition_chapter(), &content);
    let chapter_2 = copy_sample_chapter(&support::in_depth_chapter(), &content);
    // Two titles that differ only in punctuation name the same folder.
    let other_title = "Quanty Sample-Notes";
    let index = ChapterIndex::read(&chapter_2).unwrap();
    ChapterIndex {
        media_title: other_title.to_owned(),
        ..index
    }
    .write(&chapter_2)
    .unwrap();
    let media_folder = chapter_1.parent().unwrap().to_path_buf();
    let uploads_folder = content
        .join(UPLOADS_FOLDER)
        .join(media_folder.file_name().unwrap());
    fs::create_dir_all(&uploads_folder).unwrap();
    let other_file = uploads_folder.join("notes.txt");
    fs::write(&other_file, "no document's pdf").unwrap();
    let items_1 = items_of(&chapter_1);
    let items_2 = items_of(&chapter_2);
    let models = throwaway.models(StandInLlm::finding_nothing());
    let first = ingest_chapter(support::chapter_at(&chapter_1), &models, &stores)
        .await
        .unwrap();
    let second = ingest_chapter(support::chapter_at(&chapter_2), &models, &stores)
        .await
        .unwrap();
    // The media is not there any more, but the document still carries its title.
    assert!(
        stores
            .graph
            .delete_media("Quanty Sample Notes")
            .await
            .unwrap()
    );

    let summary = delete_media("Quanty Sample Notes", &content, &stores)
        .await
        .unwrap();
    assert_eq!(
        summary,
        MediaDeleteSummary {
            title: "Quanty Sample Notes".to_owned(),
            collection: collection.clone(),
            documents: vec![DocumentDeleteSummary {
                doc_id: first.doc_id,
                collection,
                points_removed: items_1.len() as u64,
                nodes_removed: items_1.len() as u64 + 1,
                folders_removed: vec![chapter_1.clone()],
                uploads_removed: Vec::new(),
            }],
            media_folders_removed: Vec::new(),
            media_node_removed: false,
        }
    );
    assert!(!chapter_1.exists());
    assert!(chapter_2.is_dir(), "the document of the other title stays");
    assert!(media_folder.is_dir(), "the media folder is not empty");
    assert!(other_file.is_file(), "the uploads folder is not empty");
    assert_document_stored(&stores.graph, &items_2).await;
    assert_eq!(point_ids(&throwaway).await, ids_of(&items_2));
    assert_ne!(first.doc_id, second.doc_id);
    let media = stores.graph.media().await.unwrap();
    let titles: Vec<&str> = media.iter().map(|media| media.title.as_str()).collect();
    assert_eq!(titles, [other_title]);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored delete::"]
async fn delete_media_removes_every_document_and_the_media_and_leaves_another_media_whole() {
    let throwaway = ThrowawayStores::new("delete-media");
    let content = throwaway.config().content_folder.clone();
    let collection = throwaway.config().items_collection.clone();
    let stores = throwaway.connect().await;
    // The upload copy of chapter 1 changes its id, so it is made before the ingest.
    let chapter_1 = copy_sample_chapter(&support::intuition_chapter(), &content);
    let chapter_2 = copy_sample_chapter(&support::in_depth_chapter(), &content);
    let chapter_other = copy_sample_chapter(&support::sample_chapter(), &content);
    let upload_1 = give_an_upload_copy(&chapter_1, &content);
    let media_folder = chapter_1.parent().unwrap().to_path_buf();
    let uploads_folder = upload_1.parent().unwrap().to_path_buf();
    let items_1 = items_of(&chapter_1);
    let items_2 = items_of(&chapter_2);
    let items_other = items_of(&chapter_other);
    let models = throwaway.models(StandInLlm::replying(|_, _| {
        Ok(json!({
            "concepts": [
                { "name": "Black–Scholes model", "definition": "a model of option prices" },
            ],
            "relations": [],
        }))
    }));
    let first = ingest_chapter(support::chapter_at(&chapter_1), &models, &stores)
        .await
        .unwrap();
    let second = ingest_chapter(support::chapter_at(&chapter_2), &models, &stores)
        .await
        .unwrap();
    ingest_chapter(support::chapter_at(&chapter_other), &models, &stores)
        .await
        .unwrap();
    let before = stored_concept_graph(&stores.graph).await;
    assert_eq!(before.concepts.len(), 1);
    let concept_points = concept_points_in(throwaway.config()).await;
    assert_eq!(concept_points.len(), 1);

    let output = run_delete_media(&throwaway, "quanty sample notes");
    assert!(output.status.success(), "{}", stderr_text(&output));

    // The documents come in the order of their ids.
    let mut documents = [
        (first.doc_id, items_1.len() as u64, &chapter_1),
        (second.doc_id, items_2.len() as u64, &chapter_2),
    ];
    documents.sort_by_key(|(id, _, _)| *id);
    let points: u64 = documents.iter().map(|(_, items, _)| items).sum();
    let mut expected = vec![
        "media: \"quanty sample notes\"".to_owned(),
        "documents removed: 2".to_owned(),
    ];
    expected.extend(
        documents
            .iter()
            .map(|(id, _, _)| format!("document id: {id}")),
    );
    expected.push(format!(
        "points removed from collection {collection}: {points}"
    ));
    expected.push(format!(
        "nodes removed from the graph: {} (the documents and their items)",
        points + 2
    ));
    expected.extend(
        documents
            .iter()
            .map(|(_, _, folder)| format!("converted folder removed: {}", folder.display())),
    );
    expected.push(format!("uploaded pdf removed: {}", upload_1.display()));
    expected.push(format!("media folder removed: {}", media_folder.display()));
    expected.push(format!(
        "media folder removed: {}",
        uploads_folder.display()
    ));
    expected.push("media removed from the graph: yes".to_owned());
    assert_eq!(stdout_lines(&output), expected);

    for id in [first.doc_id, second.doc_id] {
        assert!(stored_document(&stores.graph, id).await.is_none());
    }
    assert!(!media_folder.exists());
    assert!(!uploads_folder.exists());
    let media = stores.graph.media().await.unwrap();
    let titles: Vec<&str> = media.iter().map(|media| media.title.as_str()).collect();
    assert_eq!(titles, [support::SAMPLE_BOOK]);
    assert_document_stored(&stores.graph, &items_other).await;
    assert!(chapter_other.is_dir());
    assert_eq!(point_ids(&throwaway).await, ids_of(&items_other));
    let one_document = size_of_one_document(items_other.len());
    assert_eq!(
        size(&stores.graph).await,
        GraphSize {
            nodes: one_document.nodes + 1,
            edges: one_document.edges + items_other.len() as u64,
        },
        "the other media, its document and its items, and the concept with its mentions"
    );
    let after = stored_concept_graph(&stores.graph).await;
    assert_eq!(after.concepts, before.concepts);
    assert_eq!(after.mentions.len(), items_other.len());
    assert_eq!(
        concept_points_in(throwaway.config()).await,
        concept_points,
        "the points of the concepts stay"
    );
}
