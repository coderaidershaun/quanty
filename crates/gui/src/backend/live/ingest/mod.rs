//! Checks a chapter before it is ingested, ingests it, and reports on the services it needs.

mod preflight;
mod run;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use ocr::ChapterJob;
use ocr::content::parse_chapter_file_name;

use crate::backend::Reply;
use crate::backend::live::{LiveContext, Services};
use crate::contract::{
    ChapterLabel, Event, Failure, FailureKind, IngestRequest, RequestId, Service, ServiceState,
};

pub use preflight::preflight;
pub use run::run;

/// What every PDF starts with.
const PDF_START: &[u8] = b"%PDF-";

/// The chapter the request names, and the job that converts it: the checks that cost nothing and
/// that a check and a start share. Nothing is written and no store is asked.
fn chapter_job<S: Services>(
    cx: &LiveContext<S>,
    ingest: &IngestRequest,
) -> Result<(ChapterLabel, ChapterJob), Failure> {
    let chapter =
        parse_chapter_file_name(&file_name_of(&ingest.pdf)).map_err(|error| cx.failure(error))?;
    let label = ChapterLabel {
        number: chapter.number,
        name: chapter.name,
    };
    let job = ChapterJob::new(&ingest.book, &ingest.pdf, &cx.config().content_folder)
        .map_err(|error| cx.failure(error))?;
    check_is_a_pdf(&ingest.pdf)?;
    Ok((label, job))
}

fn file_name_of(pdf: &Path) -> String {
    pdf.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Passes a file that can be read and starts like a PDF. The size is not limited here.
fn check_is_a_pdf(pdf: &Path) -> Result<(), Failure> {
    let name = file_name_of(pdf);
    let refused = |problem: &str, detail: String| {
        let hint = format!("{name} {problem}. Choose a PDF that opens in a PDF reader.");
        Failure::new(FailureKind::BadFile, detail).with_hint(hint)
    };
    let cannot_read = |error: std::io::Error| {
        refused(
            "cannot be read",
            format!("could not read {}: {error}", pdf.display()),
        )
    };
    let file = File::open(pdf).map_err(cannot_read)?;
    if !file.metadata().map_err(cannot_read)?.is_file() {
        let detail = format!("{} is not a file", pdf.display());
        return Err(refused("is not a file", detail));
    }
    let mut start = Vec::new();
    file.take(PDF_START.len() as u64)
        .read_to_end(&mut start)
        .map_err(cannot_read)?;
    if start != PDF_START {
        let detail = format!("{} does not start with %PDF-", pdf.display());
        return Err(refused("is not a PDF", detail));
    }
    Ok(())
}

/// Not built yet. It says "not known" and not "down": while the app believes a store is down it
/// checks again after every search that works, and a check that always said "down" would keep it
/// checking for ever.
pub async fn check_health<S: Services>(_cx: &LiveContext<S>, request: RequestId, reply: &Reply) {
    for service in Service::ALL {
        reply.send(Event::Health {
            request,
            service,
            state: ServiceState::Unknown,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rag_core::Config;

    use crate::backend::live::{LiveContext, RealServices};
    use crate::backend::{Handler, Reply};
    use crate::contract::{
        AskDraft, Command, Effect, Event, Failure, FailureKind, Intent, SearchReply, Service,
        ServiceState,
    };
    use crate::state::Shared;

    /// No address here has a listener and no key is set, so this context can reach no service.
    fn context() -> LiveContext<RealServices> {
        let config = Config {
            qdrant_url: "http://127.0.0.1:1".to_owned(),
            falkordb_url: "falkor://127.0.0.1:1".to_owned(),
            falkordb_graph: "no-graph".to_owned(),
            items_collection: "no-items".to_owned(),
            concepts_collection: "no-concepts".to_owned(),
            concept_cache_folder: PathBuf::new(),
            concept_decision_log: PathBuf::new(),
            content_folder: PathBuf::new(),
            gemini_api_key: None,
            jev_api_key: None,
        };
        LiveContext::new(config, RealServices)
    }

    /// Asks a question and delivers `result` as its search. Gives what the state then asked for.
    fn search(shared: &mut Shared, result: Result<SearchReply, Failure>) -> Vec<Effect> {
        let draft = AskDraft {
            question: "q".to_owned(),
            ..AskDraft::default()
        };
        shared.apply_intent(Intent::Ask(draft), &mut Vec::new());
        let request = shared.ask.request.expect("an ask is running");
        let mut effects = Vec::new();
        shared.apply_event(Event::Search { request, result }, &mut effects);
        effects
    }

    #[tokio::test]
    async fn the_unbuilt_health_check_is_asked_once_after_a_store_failure() {
        let mut shared = Shared::default();
        let down = Failure::new(FailureKind::QdrantDown, "connection refused");
        assert!(search(&mut shared, Err(down)).is_empty());

        let effects = search(&mut shared, Ok(SearchReply::default()));
        let [Effect::Send(check @ Command::CheckHealth { .. })] = effects.as_slice() else {
            panic!("expected one health check, got {effects:?}");
        };

        let (reply, answers, _stop) = Reply::collecting();
        context().serve(check.clone(), reply).await;
        for answer in answers.try_iter() {
            shared.apply_event(answer, &mut Vec::new());
        }
        assert!(
            Service::ALL
                .iter()
                .all(|service| shared.health.of(*service) == &ServiceState::Unknown),
            "a check that is not built knows nothing: {:?}",
            shared.health
        );
        assert!(shared.is_at_rest());

        assert!(
            search(&mut shared, Ok(SearchReply::default())).is_empty(),
            "a check that learned nothing is not asked for again"
        );
    }
}
