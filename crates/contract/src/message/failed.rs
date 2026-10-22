//! Every request settles all of the replies its caller can be waiting for.
use super::{
    Balance, BalanceReads, Command, Event, Failure, ReadFailure, RequestId, Service, ServiceState,
};

impl Command {
    // SMELL: this file is near the 500-line limit, and each new command adds an arm here. This
    // function and `unknown_health` are the part to move out first, into a file of what each
    // command owes when it fails.
    /// A health check is answered with `Unknown` for every service and not with `Down`: a check
    /// that failed says nothing about the services, and `Down` would make the next good search
    /// send the check again, for ever.
    pub fn failed(&self, failure: &Failure) -> Vec<Event> {
        match self {
            Command::SaveModelChoice { request, .. } => vec![Event::ModelChoiceSaved {
                request: *request,
                result: Err(failure.clone()),
            }],
            // An ask is answered in three parts, and any of them may still be waited for.
            Command::Ask { request, .. } => vec![
                Event::Search {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::Graph {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::Answer {
                    request: *request,
                    result: Err(failure.clone()),
                },
            ],
            Command::Export { request, file } => vec![Event::Exported {
                request: *request,
                format: file.format,
                result: Err(failure.clone()),
            }],
            Command::LoadPage { request, .. } => vec![
                Event::Page {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::PageConcepts {
                    request: *request,
                    result: Err(failure.clone()),
                },
            ],
            // The concepts are owed even when the contents failed: they end the load.
            Command::LoadChapterContents { request, .. } => vec![
                Event::ChapterContents {
                    request: *request,
                    result: Err(failure.clone()),
                },
                Event::ChapterConcepts {
                    request: *request,
                    result: Err(failure.clone()),
                },
            ],
            Command::LoadCatalogue { request } => vec![Event::Catalogue {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::SetDocumentTags { request, edit } => vec![Event::DocumentTagsSaved {
                request: *request,
                doc: edit.doc,
                result: Err(failure.clone()),
            }],
            Command::DeleteDocument { request, doc } => vec![Event::DocumentDeleted {
                request: *request,
                doc: *doc,
                result: Err(failure.clone()),
            }],
            Command::SaveMedia { request, .. } => vec![Event::MediaSaved {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::EditMedia { request, .. } => vec![Event::MediaEdited {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::DeleteMedia { request, .. } => vec![Event::MediaDeleted {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::Preflight { request, .. } => vec![Event::Preflight {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::Ingest { request, .. } => vec![Event::IngestFinished {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::BuildDataset { request, .. } => vec![Event::DatasetFinished {
                request: *request,
                result: Err(failure.clone()),
            }],
            Command::CheckHealth { request } => unknown_health(*request).collect(),
            Command::LoadChapterInsights { request, .. } => vec![Event::ChapterInsights {
                request: *request,
                result: Err(failure.clone()),
            }],
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
