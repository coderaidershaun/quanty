//! The messages cross a socket without losing their values.
use contract::*;

#[test]
fn every_command_round_trips() {
    let request = RequestId(7);
    let doc = DocId::default();
    let ingest = IngestRequest {
        pdf: UploadId::default(),
        file_name: "chapter.pdf".into(),
        media: "Book".into(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 1,
            name: "Chapter".into(),
        },
        tags: vec!["test".into()],
    };
    let commands = vec![
        Command::Ask {
            request,
            ask: AskDraft::default(),
        },
        Command::Export {
            request,
            file: ExportFile {
                format: export::Format::Pdf,
                document: export::Document::default(),
                file_name: "answer.pdf".into(),
            },
        },
        Command::LoadPage {
            request,
            doc,
            page: 2,
            folder: Some("chapter".into()),
        },
        Command::LoadChapterContents {
            request,
            doc,
            folder: None,
        },
        Command::LoadCatalogue { request },
        Command::SetDocumentTags {
            request,
            edit: DocumentTagsEdit::default(),
        },
        Command::DeleteDocument { request, doc },
        Command::SaveMedia {
            request,
            media: NewMedia::default(),
        },
        Command::EditMedia {
            request,
            edit: MediaEdit::default(),
        },
        Command::DeleteMedia {
            request,
            title: "Book".into(),
        },
        Command::Preflight {
            request,
            ingest: ingest.clone(),
        },
        Command::Ingest { request, ingest },
        Command::BuildDataset {
            request,
            build: DatasetRequest::default(),
        },
        Command::CheckHealth { request },
        Command::LoadChapterInsights {
            request,
            doc,
            folder: None,
        },
        Command::Cancel(request),
        Command::ReadBalances { request },
        Command::SaveModelChoice {
            request,
            work: choices::WorkKind::Ask,
            choice: choices::ModelSettings::default(),
        },
    ];
    for command in commands {
        let json = serde_json::to_string(&command).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
    }
}

#[test]
fn every_frame_round_trips() {
    let frames = [
        Frame::Event(Event::Catalogue {
            request: RequestId(3),
            result: Ok(Catalogue::default()),
        }),
        Frame::Finished(RequestId(3)),
    ];
    for frame in frames {
        let json = serde_json::to_string(&frame).unwrap();
        assert_eq!(serde_json::from_str::<Frame>(&json).unwrap(), frame);
    }
    assert_eq!(
        serde_json::to_string(&Frame::Finished(RequestId(3))).unwrap(),
        r#"{"frame":"finished","payload":3}"#
    );
}
