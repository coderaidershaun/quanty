//! A page that fails stops the run with its number, and the next run converts only the pages that
//! are left.

use std::error::Error;

use ocr::content::ContentError;
use ocr::convert::{
    ConvertError, PageProgress, convert_chapter_with, convert_chapter_with_progress,
};
use ocr::testing::{Call, Scenario, StubServices, page_folder, read_json, sample_job};

#[tokio::test]
async fn failing_page_is_named_and_the_next_run_resumes() {
    let root = tempfile::tempdir().unwrap();
    let job = sample_job(root.path());
    let failing = StubServices::new(Scenario::AllTables {
        broken_page: Some(7),
    });
    let mut heard = Vec::new();

    let error = convert_chapter_with_progress(&job, &failing, |progress| heard.push(progress))
        .await
        .unwrap_err();

    let chapter = job.chapter_folder();
    let rejected = chapter.join("page-num-7.partial/rejected-reply.json");
    assert!(
        matches!(
            &error,
            ConvertError::PageFailed { position: 7, source, .. }
                if matches!(**source, ocr::PageError::ReplyRejected { .. })
        ),
        "{error:?}"
    );
    let mut chain = vec![error.to_string()];
    let mut source = error.source();
    while let Some(next) = source {
        chain.push(next.to_string());
        source = next.source();
    }
    let chain = chain.join(": ");
    assert!(chain.contains("page 7"), "{chain}");
    assert!(chain.contains("page-num-7.partial"), "{chain}");
    assert!(chain.contains(&rejected.display().to_string()), "{chain}");

    let transcriptions: Vec<Call> = failing
        .calls_for(7)
        .into_iter()
        .filter(|call| matches!(call, Call::Transcribe { .. }))
        .collect();
    assert_eq!(transcriptions.len(), 2);
    assert!(matches!(
        &transcriptions[0],
        Call::Transcribe {
            correction: None,
            ..
        }
    ));
    assert!(matches!(
        &transcriptions[1],
        Call::Transcribe {
            correction: Some(_),
            ..
        }
    ));
    assert!(
        std::fs::read_to_string(&rejected)
            .unwrap()
            .contains("frac{a")
    );
    for position in 1..=6 {
        assert!(page_folder(&chapter, position).join("page.json").is_file());
    }
    assert!(!page_folder(&chapter, 7).exists());
    assert_eq!(read_json(&chapter.join("chapter.json"))["finished"], false);
    let failed: Vec<&PageProgress> = heard
        .iter()
        .filter(|progress| matches!(progress, PageProgress::PageFailed { .. }))
        .collect();
    assert_eq!(failed, [&PageProgress::PageFailed { position: 7 }]);

    // A page.json that cannot be read for any reason except being missing or malformed is an
    // error to report, not a reason to delete the page and pay to convert it again.
    let page_json = page_folder(&chapter, 3).join("page.json");
    let saved_page_json = std::fs::read(&page_json).unwrap();
    std::fs::remove_file(&page_json).unwrap();
    std::fs::create_dir(&page_json).unwrap();
    let unreadable = StubServices::new(Scenario::AllTables { broken_page: None });
    let error = convert_chapter_with(&job, &unreadable).await.unwrap_err();
    assert!(
        matches!(error, ConvertError::Content(ContentError::Read { .. })),
        "{error:?}"
    );
    assert!(unreadable.calls().is_empty());
    assert!(page_folder(&chapter, 3).join("02-text.md").is_file());
    std::fs::remove_dir(&page_json).unwrap();
    std::fs::write(&page_json, saved_page_json).unwrap();

    let working = StubServices::new(Scenario::AllTables { broken_page: None });
    let mut heard = Vec::new();
    let summary = convert_chapter_with_progress(&job, &working, |progress| heard.push(progress))
        .await
        .unwrap();

    assert!(working.calls().iter().all(|call| call.position() == 7));
    assert!(!working.calls().is_empty());
    assert!(!chapter.join("page-num-7.partial").exists());
    assert_eq!(read_json(&chapter.join("chapter.json"))["finished"], true);
    assert_eq!((summary.converted_now, summary.already_done), (1, 6));
    assert!(
        matches!(
            heard.as_slice(),
            [
                PageProgress::Pages {
                    total: 7,
                    done_before: 6
                },
                PageProgress::PageDone { position: 7, .. }
            ]
        ),
        "{heard:?}"
    );
}

/// Two runs on one chapter at the same time: one converts it, and the other is refused before it
/// makes a call or removes the first one's working folders.
#[tokio::test]
async fn a_second_run_at_the_same_time_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let job = sample_job(root.path());
    let first = StubServices::new(Scenario::AllTables { broken_page: None });
    let second = StubServices::new(Scenario::AllTables { broken_page: None });

    let (first_run, second_run) = tokio::join!(
        convert_chapter_with(&job, &first),
        convert_chapter_with(&job, &second)
    );

    let (summary, refused, refused_stubs) = match (first_run, second_run) {
        (Ok(summary), Err(refused)) => (summary, refused, &second),
        (Err(refused), Ok(summary)) => (summary, refused, &first),
        both => panic!("one run should finish and the other be refused: {both:?}"),
    };
    assert!(
        matches!(refused, ConvertError::ChapterBusy { .. }),
        "{refused:?}"
    );
    assert!(refused_stubs.calls().is_empty());
    assert_eq!(summary.converted_now, 7);
    let chapter = job.chapter_folder();
    assert_eq!(read_json(&chapter.join("chapter.json"))["finished"], true);
}
