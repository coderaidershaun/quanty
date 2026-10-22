//! Backend failures retain useful context across the wire.

use std::io;
use std::path::PathBuf;

use backend::live::IntoFailure;
use contract::FailureKind as Kind;
use ocr::ContentError;
use rag_ingestion::DeleteError;

#[test]
fn every_error_of_a_delete_says_what_a_person_can_do() {
    let folder = PathBuf::from("/content/notes/chapter-1");
    let id = rag_core::DocId::from_source_sha256("a made-up hash");
    let rows = [
        (
            DeleteError::Converting {
                folder: folder.clone(),
            },
            Kind::ChapterTaken,
            vec!["/content/notes/chapter-1", "delete again"],
        ),
        (
            DeleteError::UnknownDocument {
                id,
                collection: "items".to_owned(),
                content_folder: folder.clone(),
            },
            Kind::SourceMissing,
            vec!["read again"],
        ),
        (
            DeleteError::UnknownMedia {
                title: "Quanty Notes".to_owned(),
            },
            Kind::SourceMissing,
            vec!["\"Quanty Notes\"", "read again"],
        ),
        (
            DeleteError::Content(ContentError::Read {
                path: folder.clone(),
                source: io::Error::other("no"),
            }),
            Kind::Internal,
            vec!["/content/notes/chapter-1", "delete again"],
        ),
        (
            DeleteError::Remove {
                path: folder,
                source: io::Error::other("no"),
            },
            Kind::Internal,
            vec!["/content/notes/chapter-1", "delete again"],
        ),
    ];
    for (error, kind, words) in rows {
        let failure = error.into_failure();
        assert_eq!(failure.kind, kind, "{failure:?}");
        for word in words {
            assert!(
                failure.hint.contains(word),
                "`{word}` is not in {failure:?}"
            );
        }
        assert_ne!(failure.hint, Kind::Internal.hint(), "{failure:?}");
    }
}
