//! Every request settles all of the replies its caller can be waiting for.
use super::{
    Balance, BalanceReads, Command, Event, Failure, ReadFailure, RequestId, Service, ServiceState,
};

impl Command {
    /// A health check is answered with `Unknown` for every service and not with `Down`: a check
    /// that failed says nothing about the services, and `Down` would make the next good search
    /// send the check again, for ever.
    pub fn failed(&self, failure: &Failure) -> Vec<Event> {
        macro_rules! failure_of {
            ($event:ident, $request:expr $(, $field:ident: $value:expr)*) => {
                Event::$event {
                    request: *$request,
                    $($field: $value,)*
                    result: Err(failure.clone()),
                }
            };
        }
        match self {
            Command::SaveModelChoice { request, .. } => {
                vec![failure_of!(ModelChoiceSaved, request)]
            }
            // An ask is answered in three parts, and any of them may still be waited for.
            Command::Ask { request, .. } => vec![
                failure_of!(Search, request),
                failure_of!(Graph, request),
                failure_of!(Answer, request),
            ],
            Command::Export { request, file } => {
                vec![failure_of!(Exported, request, format: file.format)]
            }
            Command::LoadPage { request, .. } => vec![
                failure_of!(Page, request),
                failure_of!(PageConcepts, request),
            ],
            // The concepts are owed even when the contents failed: they end the load.
            Command::LoadChapterContents { request, .. } => vec![
                failure_of!(ChapterContents, request),
                failure_of!(ChapterConcepts, request),
            ],
            Command::LoadCatalogue { request } => vec![failure_of!(Catalogue, request)],
            Command::SetDocumentTags { request, edit } => {
                vec![failure_of!(DocumentTagsSaved, request, doc: edit.doc)]
            }
            Command::DeleteDocument { request, doc } => {
                vec![failure_of!(DocumentDeleted, request, doc: *doc)]
            }
            Command::SaveMedia { request, .. } => vec![failure_of!(MediaSaved, request)],
            Command::EditMedia { request, .. } => vec![failure_of!(MediaEdited, request)],
            Command::DeleteMedia { request, .. } => vec![failure_of!(MediaDeleted, request)],
            Command::Preflight { request, .. } => vec![failure_of!(Preflight, request)],
            Command::Ingest { request, .. } => vec![failure_of!(IngestFinished, request)],
            Command::BuildDataset { request, .. } => vec![failure_of!(DatasetFinished, request)],
            Command::CheckHealth { request } => unknown_health(*request).collect(),
            Command::LoadChapterInsights { request, .. } => {
                vec![failure_of!(ChapterInsights, request)]
            }
            Command::Cancel(_) => Vec::new(),
            // A read that broke is answered, so the footer does not wait for it for ever.
            Command::ReadBalances { request } => {
                let unread = Balance::Unread(ReadFailure::Internal);
                vec![Event::Balances {
                    request: *request,
                    found: BalanceReads {
                        google: unread,
                        jev: unread,
                    },
                }]
            }
        }
    }
}

/// Every health check that learnt nothing about the services gives this answer.
pub fn unknown_health(request: RequestId) -> impl Iterator<Item = Event> {
    Service::ALL.into_iter().map(move |service| Event::Health {
        request,
        service,
        state: ServiceState::Unknown,
    })
}
