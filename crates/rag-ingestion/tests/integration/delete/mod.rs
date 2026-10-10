//! Runs the delete of a document and of a media against throwaway stores and a throwaway content
//! folder, to show what goes and what stays. Every delete gets the content folder of its own
//! throwaway stores and no other.

mod document;
mod whole_media;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use ocr::{ChapterIndex, ChapterJob, MediaDocument, read_chapter};
use rag_ingestion::{DocumentDeleteSummary, Item, chapter_items};

use crate::support::{RunRagIngest, ThrowawayStores, points_in};

const UPLOADS_FOLDER: &str = "_uploads";

fn items_of(chapter_folder: &Path) -> Vec<Item> {
    chapter_items(&read_chapter(chapter_folder).unwrap())
}

fn run_delete_document(throwaway: &ThrowawayStores, id: &str) -> Output {
    throwaway.rag_ingest(["delete-document", id])
}

fn run_delete_media(throwaway: &ThrowawayStores, title: &str) -> Output {
    throwaway.rag_ingest(["delete-media", title])
}

fn stdout_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The lines the command prints for this summary, written out here so that a change to what a
/// person reads is seen.
fn printed_lines(summary: &DocumentDeleteSummary) -> Vec<String> {
    let mut lines = vec![
        format!("document id: {}", summary.doc_id),
        format!(
            "points removed from collection {}: {}",
            summary.collection, summary.points_removed
        ),
        format!(
            "nodes removed from the graph: {} (the document and its items)",
            summary.nodes_removed
        ),
    ];
    lines.extend(path_lines(
        "converted folder removed",
        &summary.folders_removed,
    ));
    lines.extend(path_lines("uploaded pdf removed", &summary.uploads_removed));
    lines
}

fn path_lines(label: &str, paths: &[PathBuf]) -> Vec<String> {
    if paths.is_empty() {
        return vec![format!("{label}: none")];
    }
    paths
        .iter()
        .map(|path| format!("{label}: {}", path.display()))
        .collect()
}

async fn point_ids(throwaway: &ThrowawayStores) -> BTreeSet<String> {
    points_in(throwaway.config())
        .await
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

fn ids_of(items: &[Item]) -> BTreeSet<String> {
    items.iter().map(|item| item.id.to_string()).collect()
}

/// Copies a sample chapter folder, `<media folder>/<document folder>`, to the same two folder
/// names under the content folder. It copies and never links, so a delete cannot reach the
/// samples. Returns the folder of the copy.
fn copy_sample_chapter(sample: &Path, content_folder: &Path) -> PathBuf {
    let document_folder = sample.file_name().unwrap();
    let media_folder = sample.parent().unwrap().file_name().unwrap();
    let copy = content_folder.join(media_folder).join(document_folder);
    copy_folder(sample, &copy);
    copy
}

fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Saves a file with these bytes where the upload copy of the PDF of this chapter is kept:
/// `<content>/_uploads/<media folder>/<source file>`.
fn write_upload(chapter_folder: &Path, content_folder: &Path, bytes: &[u8]) -> PathBuf {
    let media_folder: &OsStr = chapter_folder.parent().unwrap().file_name().unwrap();
    let source_file = ChapterIndex::read(chapter_folder).unwrap().source_file;
    let upload_folder = content_folder.join(UPLOADS_FOLDER).join(media_folder);
    fs::create_dir_all(&upload_folder).unwrap();
    let upload = upload_folder.join(source_file);
    fs::write(&upload, bytes).unwrap();
    upload
}

/// Saves a small file as the upload copy of the PDF of this chapter, and writes the hash of that
/// file into the index of the chapter, so the file is the PDF that the chapter was made from.
/// The document id comes from that hash, so call it before the ingest, and take the items from
/// the copy after it, never from the sample.
fn give_an_upload_copy(chapter_folder: &Path, content_folder: &Path) -> PathBuf {
    let index = ChapterIndex::read(chapter_folder).unwrap();
    let upload = write_upload(chapter_folder, content_folder, b"the pdf of the chapter");
    let document = MediaDocument {
        media_title: index.media_title.clone(),
        name: index.name.clone(),
    };
    let job = ChapterJob::new(document, &upload, content_folder).unwrap();
    ChapterIndex {
        source_sha256: job.source_sha256().unwrap(),
        ..index
    }
    .write(chapter_folder)
    .unwrap();
    upload
}
