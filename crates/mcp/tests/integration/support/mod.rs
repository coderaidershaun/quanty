//! What the tests share: stand-in services, settings whose stores cannot be reached, and an
//! in-process MCP client.

mod client;
mod library;
mod pdf_host;
mod reading;
mod services;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rag_core::DocId;
use rmcp::RoleClient;
use rmcp::service::{RunningService, Service};
use serde_json::{Value, json};

pub use client::{
    Client, ProgressRecorder, call, call_and_cancel, connect, connect_declaring_tasks,
    connect_on_2026_07_28, connect_recording_progress, error_text, structured, tasks_client_config,
};
pub use library::{AUTHOR, ClosedPorts, Library, QUESTION};
pub use pdf_host::{PdfHost, Reply};
pub use reading::{
    chars, counted, dataset_sections, headings_of, listed_and_read_back_items, long_notes_copy,
    parts_of,
};
pub use services::{ReadyGate, StandInServices};

/// The committed sample files of the workspace. Tests only read them.
pub fn samples_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples")
}

pub fn sample_content_folder() -> PathBuf {
    samples_folder().join("content")
}

/// The chapter that the sample PDF is converted to, which has seven pages.
pub fn sample_chapter_folder() -> PathBuf {
    sample_content_folder().join("option-volatility-and-pricing/chapter-1")
}

pub fn sample_document_id() -> DocId {
    let index = ocr::ChapterIndex::read(&sample_chapter_folder())
        .expect("the sample chapter should have a chapter.json");
    DocId::from_source_sha256(&index.source_sha256)
}

/// Copies the committed sample chapters into `to`, so that a test which writes a file works on its
/// own copy and never on the samples. The copy leaves out every `insights.json`, so that every
/// chapter starts with none whether or not a sample has one.
pub fn copy_samples(to: &Path) {
    fn copy_folder(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("the copy of a sample folder should be made");
        for entry in std::fs::read_dir(from).expect("a sample folder should be listed") {
            let entry = entry.expect("a sample entry should be read");
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy_folder(&entry.path(), &target);
            } else if entry.file_name() != "insights.json" {
                std::fs::copy(entry.path(), &target).expect("a sample file should be copied");
            }
        }
    }
    copy_folder(&sample_content_folder(), to);
}

/// The document id of a chapter of the committed notes media, which has three pages in its first
/// chapter and three in its second.
pub fn sample_notes_document_id(chapter: u32) -> DocId {
    let folder = sample_content_folder().join(format!("quanty-sample-notes/chapter-{chapter}"));
    let index = ocr::ChapterIndex::read(&folder)
        .expect("a chapter of the sample notes should have a chapter.json");
    DocId::from_source_sha256(&index.source_sha256)
}

/// How long a job may take to end in a test: seven pages of stand-in calls and an ingest, or a
/// build of the sample chapters.
const JOB_WAIT: Duration = Duration::from_secs(180);

/// Asks `status_tool` (`job_status`, or `ingest_status`, `insights_status` or `dataset_status`)
/// about the job until it is not running.
pub async fn report_when_ended<S: Service<RoleClient>>(
    client: &RunningService<RoleClient, S>,
    status_tool: &'static str,
    job_id: &str,
) -> Value {
    let started = Instant::now();
    loop {
        let result = call(client, status_tool, json!({ "job_id": job_id })).await;
        let report = structured(&result).clone();
        if report["state"] != "running" {
            return report;
        }
        assert!(
            started.elapsed() < JOB_WAIT,
            "the job did not end in time: {report}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// How long a test waits for a job to come to a stage it expects, so that a failure does not hang.
pub const STAGE_WAIT: Duration = Duration::from_secs(60);

/// Asks `job_status` until the job is in the stage called `name`.
pub async fn status_in_stage<S: Service<RoleClient>>(
    client: &RunningService<RoleClient, S>,
    job_id: &str,
    name: &str,
) -> Value {
    let started = Instant::now();
    loop {
        let status =
            structured(&call(client, "job_status", json!({ "job_id": job_id })).await).clone();
        if status["stage"]["name"] == name {
            return status;
        }
        assert!(
            started.elapsed() < STAGE_WAIT,
            "the job did not come to the stage {name}: {status}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
