//! Runs `rag-ingest media` as a person would, against throwaway stores, to show that it can take
//! every author and every tag away from a stored media.

use graph::{GraphStore, MediaNode};
use rag_core::{Category, MediaLabels};

use crate::support::{RunRagIngest, ThrowawayStores};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored media::"]
async fn the_media_command_takes_every_author_and_every_tag_away() {
    let throwaway = ThrowawayStores::new("media-command");
    let stores = throwaway.connect().await;
    let paper = MediaLabels {
        category: Category::Paper,
        authors: vec!["A. Author".to_owned()],
        tags: ["hawkes".parse().unwrap()].into(),
    };
    let media = MediaNode {
        title: "Hawkes Processes in Finance".to_owned(),
        labels: paper,
    };
    stores.graph.add_media(&media).await.unwrap();

    let output = throwaway.rag_ingest([
        "media",
        "hawkes processes in finance",
        "--no-authors",
        "--no-tags",
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let emptied = MediaLabels {
        category: Category::Paper,
        ..MediaLabels::default()
    };
    assert_eq!(
        stores.graph.media().await.unwrap(),
        vec![MediaNode {
            title: "Hawkes Processes in Finance".to_owned(),
            labels: emptied,
        }]
    );
}
