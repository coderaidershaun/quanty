//! Checks that the live backend checks and runs an ingest of a PDF of a saved media. The helpers
//! here send one command and read the events that come back.

mod blocked;
mod run;

use backend::live::{LiveContext, Services};
use backend::{Handler, Reply};
use contract::{
    Category, Command, DocumentName, Event, Failure, IngestOutcome, IngestProgress, IngestRequest,
    Preflight, RequestId, UploadId,
};
use ocr::testing::sample_pdf;

const REQUEST: RequestId = RequestId(7);
const SAMPLE_BOOK: &str = "Option Volatility and Pricing";

fn request<S: Services>(cx: &LiveContext<S>) -> IngestRequest {
    IngestRequest {
        pdf: upload(cx, &sample_pdf()),
        file_name: sample_pdf()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        media: SAMPLE_BOOK.to_owned(),
        category: Category::Book,
        name: DocumentName::Chapter {
            number: 1,
            name: "Sample Pages".to_owned(),
        },
        tags: Vec::new(),
    }
}

async fn events_of<S: Services>(cx: &LiveContext<S>, command: Command) -> Vec<Event> {
    let (reply, events, _stop) = Reply::collecting();
    cx.serve(command, reply).await;
    events.try_iter().collect()
}

async fn checked<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<Preflight, Failure> {
    let command = Command::Preflight {
        request: REQUEST,
        ingest: ingest.clone(),
    };
    let events = events_of(cx, command).await;
    let [
        Event::Preflight {
            request: REQUEST,
            result,
        },
    ] = events.as_slice()
    else {
        panic!("expected one preflight for {REQUEST:?}: {events:#?}");
    };
    result.clone()
}

async fn started<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> (Vec<IngestProgress>, Result<IngestOutcome, Failure>) {
    let command = Command::Ingest {
        request: REQUEST,
        ingest: ingest.clone(),
    };
    let events = events_of(cx, command).await;
    let Some((
        Event::IngestFinished {
            request: REQUEST,
            result,
        },
        before,
    )) = events.split_last()
    else {
        panic!("the last event must end the ingest of {REQUEST:?}: {events:#?}");
    };
    let progress = before
        .iter()
        .map(|event| match event {
            Event::IngestProgress {
                request: REQUEST,
                progress,
            } => progress.clone(),
            other => panic!("only progress comes before the end: {other:#?}"),
        })
        .collect();
    (progress, result.clone())
}

fn upload<S: Services>(cx: &LiveContext<S>, pdf: &std::path::Path) -> UploadId {
    let upload = UploadId(uuid::Uuid::new_v4());
    let folder = cx.home().join("data/uploads").join(upload.0.to_string());
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(pdf, folder.join(pdf.file_name().unwrap())).unwrap();
    upload
}

fn assert_consumed<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
    source: &std::path::Path,
) {
    let temporary = cx
        .home()
        .join("data/uploads")
        .join(ingest.pdf.0.to_string());
    assert!(
        !temporary.exists(),
        "the upload folder is removed before ingestion"
    );
    let media = ocr::content::media_folder_name(&ingest.media).unwrap();
    let saved = cx
        .config()
        .content_folder
        .join("_uploads")
        .join(media)
        .join(&ingest.file_name);
    assert_eq!(
        std::fs::read(saved).unwrap(),
        std::fs::read(source).unwrap(),
        "the uploaded PDF persists with the content"
    );
}
