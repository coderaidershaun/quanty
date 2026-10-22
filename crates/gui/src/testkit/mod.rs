//! What a test needs to look at one panel or the whole app with no backend: a window around a
//! panel, the app on the fake backend, builders for the shared state, and small sample values.

mod folder_fetch;
mod harness;
mod in_process;
pub use folder_fetch::FolderFetch;
pub use in_process::InProcess;
pub mod sample;

use std::path::PathBuf;

use crate::state::Shared;
use contract::{Answer, AskDraft, ConceptGraph, Event, Intent, SearchReply};

pub use harness::{
    Host, app, app_at, app_at_with_home, app_on, panel, run_until, save_png, settle, settle_within,
};

const QUESTION: &str = "How is the Black–Scholes formula derived?";

pub fn samples_folder() -> PathBuf {
    harness::repository_root().join("samples").join("content")
}

pub fn asked(question: &str) -> Shared {
    let mut shared = Shared::default();
    let draft = AskDraft {
        question: question.to_owned(),
        ..AskDraft::default()
    };
    shared.apply_intent(Intent::Ask(draft), &mut Vec::new());
    shared
}

pub fn searched(reply: SearchReply) -> Shared {
    let mut shared = asked(QUESTION);
    deliver(&mut shared, |request| Event::Search {
        request,
        result: Ok(reply),
    });
    shared
}

pub fn answered(reply: SearchReply, graph: ConceptGraph, answer: Answer) -> Shared {
    let mut shared = searched(reply);
    deliver(&mut shared, |request| Event::Graph {
        request,
        result: Ok(graph),
    });
    deliver(&mut shared, |request| Event::Answer {
        request,
        result: Ok(answer),
    });
    shared
}

fn deliver(shared: &mut Shared, event: impl FnOnce(contract::RequestId) -> Event) {
    let request = shared
        .ask
        .request
        .expect("an ask was made before the reply");
    shared.apply_event(event(request), &mut Vec::new());
}
