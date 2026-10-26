//! Checks that the live backend reads the saved insights of a chapter from disk with no store and
//! no model.

use std::path::{Path, PathBuf};

use backend::Handler;
use contract::{ChapterInsights, Command, DocId, Event, Failure, FailureKind, Insight, RequestId};
use uuid::Uuid;

use crate::support::{IN_DEPTH, closed, copy_folder, document_of, sample_chapter, served};

const REQUEST: RequestId = RequestId(7);

#[tokio::test(flavor = "multi_thread")]
async fn the_live_backend_reads_the_saved_insights_with_the_stores_down() {
    let content = tempfile::tempdir().expect("a temporary folder should be made");
    let copy = content.path().join(IN_DEPTH);
    copy_folder(&sample_chapter(IN_DEPTH), &copy);
    let doc = document_of(&copy);
    let cx = closed(content.path());
    let expected = expected_from(&copy);
    assert!(
        !expected.insights.is_empty(),
        "the sample chapter carries insights"
    );

    let kept = insights_of(&cx, doc, Some(copy.clone())).await;
    assert_eq!(kept.expect("the file is read"), Some(expected.clone()));
    let scanned = insights_of(&cx, doc, None).await;
    assert_eq!(
        scanned.expect("the file is found by the scan"),
        Some(expected)
    );

    let file = copy.join(ocr::content::INSIGHTS_FILE);
    std::fs::write(&file, "not json").expect("the file is overwritten");
    let failure = insights_of(&cx, doc, Some(copy.clone()))
        .await
        .expect_err("a file that is not JSON cannot be read");
    assert_eq!(failure.kind, FailureKind::BadFile);
    assert!(
        failure
            .hint
            .contains(&format!("rag-ingest insights --document {}", doc.0)),
        "{failure:?}"
    );
    assert!(
        failure.detail.contains(&file.display().to_string()),
        "{failure:?}"
    );

    std::fs::remove_file(&file).expect("the file is removed");
    let none = insights_of(&cx, doc, Some(copy)).await;
    assert_eq!(none.expect("a missing file is not an error"), None);

    let unknown = insights_of(&cx, DocId(Uuid::from_u128(9)), None)
        .await
        .expect_err("a document with no folder has no insights to read");
    assert_eq!(unknown.kind, FailureKind::SourceMissing);
}

/// The answer to one `LoadChapterInsights`, which is the only event it sends.
pub(super) async fn insights_of(
    backend: &impl Handler,
    doc: DocId,
    folder: Option<PathBuf>,
) -> Result<Option<ChapterInsights>, Failure> {
    let command = Command::LoadChapterInsights {
        request: REQUEST,
        doc,
        folder,
    };
    let events = served(backend, command).await;
    let [
        Event::ChapterInsights {
            request: REQUEST,
            result,
        },
    ] = events.as_slice()
    else {
        panic!("expected the insights of {doc:?} and nothing else: {events:#?}");
    };
    result.clone()
}

/// What the converter's reader finds in the folder, written out field by field.
fn expected_from(folder: &Path) -> ChapterInsights {
    let saved = ocr::ChapterInsights::read(folder)
        .expect("the sample file is read")
        .expect("the sample file is there");
    ChapterInsights {
        model: saved.model,
        insights: saved
            .insights
            .into_iter()
            .map(|insight| Insight {
                text: insight.text,
                pages: insight.pages,
            })
            .collect(),
        takeaways: saved.takeaways,
        cheat_sheet: saved.cheat_sheet,
    }
}
