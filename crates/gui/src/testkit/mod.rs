//! What a test needs to look at one panel or the whole app with no backend: a window around a
//! panel, the app on the fake backend, builders for the shared state, and small sample values.

mod harness;
pub mod sample;

use std::path::PathBuf;

use crate::contract::{
    Answer, AskDraft, ConceptGraph, Event, Intent, SearchReply, Service, ServiceState,
};
use crate::state::Shared;

pub use harness::{Host, app, app_on, panel, save_png, settle, settle_within};

const QUESTION: &str = "How is the Black–Scholes formula derived?";

/// The folder of the committed sample chapters.
pub fn samples_folder() -> PathBuf {
    harness::repository_root().join("samples").join("content")
}

/// The state after a question was asked: the search is loading.
pub fn asked(question: &str) -> Shared {
    let mut shared = Shared::default();
    let draft = AskDraft {
        question: question.to_owned(),
        ..AskDraft::default()
    };
    shared.apply_intent(Intent::Ask(draft), &mut Vec::new());
    shared
}

/// The state after the search came back: the results are shown and the answer is loading.
pub fn searched(reply: SearchReply) -> Shared {
    let mut shared = asked(QUESTION);
    deliver(&mut shared, |request| Event::Search {
        request,
        result: Ok(reply),
    });
    shared
}

/// The state after the search, the graph and the answer all came back.
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

/// The state with one service in this state and every other service up.
pub fn with_health(mut shared: Shared, service: Service, state: ServiceState) -> Shared {
    shared.apply_intent(Intent::RecheckHealth, &mut Vec::new());
    let request = shared
        .health
        .pending
        .expect("a check was started by RecheckHealth");
    for each in Service::ALL {
        let state = if each == service {
            state.clone()
        } else {
            ServiceState::Up {
                detail: "ready".to_owned(),
            }
        };
        let event = Event::Health {
            request,
            service: each,
            state,
        };
        shared.apply_event(event, &mut Vec::new());
    }
    shared
}

fn deliver(shared: &mut Shared, event: impl FnOnce(crate::contract::RequestId) -> Event) {
    let request = shared
        .ask
        .request
        .expect("an ask was made before the reply");
    shared.apply_event(event(request), &mut Vec::new());
}
