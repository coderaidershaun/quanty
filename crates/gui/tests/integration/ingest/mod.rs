//! Checks that the live backend checks and runs an ingest of a PDF of a saved media. The helpers
//! here send one command and read the events that come back.

mod blocked;
mod run;

use gui::backend::live::{LiveContext, Services};
use gui::backend::{Handler, Reply};
use gui::contract::{
    Category, Command, DocumentName, Event, Failure, IngestOutcome, IngestProgress, IngestRequest,
    Preflight, RequestId,
};
use ocr::testing::sample_pdf;

const REQUEST: RequestId = RequestId(7);
const SAMPLE_BOOK: &str = "Option Volatility and Pricing";

fn request() -> IngestRequest {
    IngestRequest {
        pdf: sample_pdf(),
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
