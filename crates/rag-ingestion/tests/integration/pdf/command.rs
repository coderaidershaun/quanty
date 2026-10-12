//! Runs `rag-ingest pdf` as a person would, against throwaway stores: the pdf is named by the
//! category that the library has for its media, so a name that breaks its rule is refused before
//! anything is converted.

use std::ffi::OsStr;

use graph::MediaNode;
use ocr::testing::sample_pdf;
use rag_core::MediaLabels;

use crate::support::{RunRagIngest, ThrowawayStores};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the local Qdrant and FalkorDB from docker compose and bills nothing; run with: cargo test -p rag-ingestion --test integration -- --ignored pdf::"]
async fn the_command_names_a_pdf_by_the_category_that_the_library_has_for_its_media() {
    let throwaway = ThrowawayStores::new("pdf-command");
    let stores = throwaway.connect().await;
    // A media with no category of its own is a book.
    let book = MediaNode {
        title: "Hawkes Processes in Finance".to_owned(),
        labels: MediaLabels::default(),
    };
    stores.graph.add_media(&book).await.unwrap();
    // A paper's PDF can have any name, and a book's cannot.
    let plain = throwaway.temporary_folder().join("hawkes-notes.pdf");
    std::fs::copy(sample_pdf(), &plain).unwrap();

    let output = throwaway.rag_ingest_with_fake_keys([
        OsStr::new("pdf"),
        OsStr::new("--paper"),
        OsStr::new("hawkes processes in finance"),
        plain.as_os_str(),
    ]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains(
            "the library has \"hawkes processes in finance\" in the category book, so its pdf must be named chapter-<number>-<name>.pdf; rename"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("give --title"), "{stderr}");
    let media_folder = throwaway
        .config()
        .content_folder
        .join("hawkes-processes-in-finance");
    assert!(!media_folder.exists(), "nothing was converted");

    // No file is at this path on purpose: the run stops at that check, before anything is
    // converted or paid for, and it still shows that --book takes --title.
    let absent = throwaway.temporary_folder().join("index.pdf");
    let output = throwaway.rag_ingest_with_fake_keys([
        OsStr::new("pdf"),
        OsStr::new("--book"),
        OsStr::new("Hawkes Processes in Finance"),
        OsStr::new("--title"),
        OsStr::new("Index"),
        absent.as_os_str(),
    ]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("is not there; give the pdf to ingest"),
        "{stderr}"
    );
}
