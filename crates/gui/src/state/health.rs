//! What is known about the six services, and the one word that sums it up.

use super::shared::{Shared, push_cancel};
use crate::contract::{
    Command, Effect, Failure, FailureKind, RequestId, Service, ServiceState, StartupFacts,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Health {
    pub facts: StartupFacts,
    /// `Some` while a check is running.
    pub pending: Option<RequestId>,
    states: [ServiceState; 6],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HealthLevel {
    Unknown,
    Checking,
    Ready,
    Limited,
    Down,
}

impl Health {
    pub(super) fn new(facts: StartupFacts) -> Health {
        let mut health = Health {
            facts,
            ..Health::default()
        };
        if health.facts.anthropic_api_key_set {
            health.states[slot(Service::Claude)] = ServiceState::Down(Failure::new(
                FailureKind::ClaudeApiKeySet,
                "the ANTHROPIC_API_KEY variable is set",
            ));
        }
        health
    }

    pub fn of(&self, service: Service) -> &ServiceState {
        &self.states[slot(service)]
    }

    /// `Down` when no ask can work: a store or the embedding key is down. `Limited` when some
    /// other service is down.
    pub fn level(&self) -> HealthLevel {
        let is_down = |service| matches!(self.of(service), ServiceState::Down(_));
        let any = |state: &ServiceState| self.states.contains(state);
        if [Service::Qdrant, Service::FalkorDb, Service::EmbeddingKey]
            .into_iter()
            .any(is_down)
        {
            HealthLevel::Down
        } else if Service::ALL.into_iter().any(is_down) {
            HealthLevel::Limited
        } else if any(&ServiceState::Checking) {
            HealthLevel::Checking
        } else if any(&ServiceState::Unknown) {
            HealthLevel::Unknown
        } else {
            HealthLevel::Ready
        }
    }

    /// While `ANTHROPIC_API_KEY` is set, nothing changes the state of Claude.
    fn set(&mut self, service: Service, state: ServiceState) {
        if service == Service::Claude && self.facts.anthropic_api_key_set {
            return;
        }
        self.states[slot(service)] = state;
    }
}

/// The place of a service in the list of states. It follows the order of `Service::ALL`.
fn slot(service: Service) -> usize {
    service as usize
}

impl Shared {
    pub(super) fn recheck_health(&mut self, effects: &mut Vec<Effect>) {
        let request = self.issue_request();
        if let Some(old) = self.health.pending {
            push_cancel(effects, old);
        }
        for service in Service::ALL {
            self.health.set(service, ServiceState::Checking);
        }
        self.health.pending = Some(request);
        effects.push(Effect::Send(Command::CheckHealth { request }));
    }

    pub(super) fn health_arrived(
        &mut self,
        request: RequestId,
        service: Service,
        state: ServiceState,
    ) {
        if self.health.pending != Some(request) {
            return;
        }
        self.health.set(service, state);
        if !self.health.states.contains(&ServiceState::Checking) {
            self.health.pending = None;
        }
    }

    /// Marks the service that a failure names as down.
    pub(super) fn mark_down(&mut self, failure: &Failure) {
        if let Some(service) = failure.service() {
            self.health
                .set(service, ServiceState::Down(failure.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{AskDraft, Command, Event, FailureKind, Intent, SearchReply};

    fn run(shared: &mut Shared, intent: Intent) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_intent(intent, &mut effects);
        effects
    }

    fn deliver(shared: &mut Shared, event: Event) -> Vec<Effect> {
        let mut effects = Vec::new();
        shared.apply_event(event, &mut effects);
        effects
    }

    fn checking(effects: &[Effect]) -> RequestId {
        match effects.last() {
            Some(Effect::Send(Command::CheckHealth { request })) => *request,
            other => panic!("expected a health check, got {other:?}"),
        }
    }

    fn answer_all(
        shared: &mut Shared,
        request: RequestId,
        state: impl Fn(Service) -> ServiceState,
    ) {
        for service in Service::ALL {
            let state = state(service);
            deliver(
                shared,
                Event::Health {
                    request,
                    service,
                    state,
                },
            );
        }
    }

    fn up(_: Service) -> ServiceState {
        ServiceState::Up {
            detail: "ready".to_owned(),
        }
    }

    fn asked(shared: &mut Shared) -> RequestId {
        let draft = AskDraft {
            question: "q".to_owned(),
            ..AskDraft::default()
        };
        match run(shared, Intent::Ask(draft)).last() {
            Some(Effect::Send(Command::Ask { request, .. })) => *request,
            other => panic!("expected an ask, got {other:?}"),
        }
    }

    #[test]
    fn a_failure_marks_its_service_down_and_a_set_api_key_keeps_claude_down() {
        let mut shared = Shared::default();
        assert_eq!(shared.health.level(), HealthLevel::Unknown);

        let first = checking(&run(&mut shared, Intent::RecheckHealth));
        assert!(
            Service::ALL
                .iter()
                .all(|service| shared.health.of(*service) == &ServiceState::Checking)
        );
        assert_eq!(shared.health.level(), HealthLevel::Checking);
        let effects = run(&mut shared, Intent::RecheckHealth);
        let second = checking(&effects);
        assert_eq!(effects[0], Effect::Send(Command::Cancel(first)));

        deliver(
            &mut shared,
            Event::Health {
                request: first,
                service: Service::Qdrant,
                state: ServiceState::Up {
                    detail: "old".to_owned(),
                },
            },
        );
        assert_eq!(
            shared.health.of(Service::Qdrant),
            &ServiceState::Checking,
            "an old answer is dropped"
        );

        deliver(
            &mut shared,
            Event::Health {
                request: second,
                service: Service::Qdrant,
                state: up(Service::Qdrant),
            },
        );
        assert_eq!(
            shared.health.pending,
            Some(second),
            "others are still being checked"
        );
        answer_all(&mut shared, second, up);
        assert_eq!(shared.health.pending, None);
        assert_eq!(shared.health.level(), HealthLevel::Ready);

        // A service that is down by itself limits the app. A store or the embedding key stops it.
        let third = checking(&run(&mut shared, Intent::RecheckHealth));
        let down = |kind| Failure::new(kind, "not ready");
        answer_all(&mut shared, third, |service| match service {
            Service::Poppler => ServiceState::Down(down(FailureKind::PopplerMissing)),
            _ => up(service),
        });
        assert_eq!(shared.health.level(), HealthLevel::Limited);

        // A failed search that names a store marks that store down, and the dot says so.
        let ask = asked(&mut shared);
        deliver(
            &mut shared,
            Event::Search {
                request: ask,
                result: Err(down(FailureKind::QdrantDown)),
            },
        );
        assert!(
            matches!(shared.health.of(Service::Qdrant), ServiceState::Down(failure) if failure.kind == FailureKind::QdrantDown)
        );
        assert_eq!(shared.health.level(), HealthLevel::Down);

        // A failure that names no service changes nothing.
        let ask = asked(&mut shared);
        let before = shared.health.clone();
        deliver(
            &mut shared,
            Event::Search {
                request: ask,
                result: Err(Failure::internal("odd")),
            },
        );
        assert_eq!(shared.health, before);

        // A search that works while the dot says Down checks the services again.
        let ask = asked(&mut shared);
        let effects = deliver(
            &mut shared,
            Event::Search {
                request: ask,
                result: Ok(SearchReply::default()),
            },
        );
        let fourth = checking(&effects);
        assert_eq!(shared.health.pending, Some(fourth));
        answer_all(&mut shared, fourth, up);
        assert_eq!(shared.health.level(), HealthLevel::Ready);

        // A failure of the answer model marks Claude down.
        let ask = asked(&mut shared);
        deliver(
            &mut shared,
            Event::Search {
                request: ask,
                result: Ok(SearchReply {
                    results: vec![crate::contract::ResultItem {
                        number: 1,
                        id: Default::default(),
                        kind: crate::contract::ItemKind::Chunk,
                        score: 0.0,
                        reason: crate::contract::Reason::Nearest,
                        doc: Default::default(),
                        doc_title: String::new(),
                        book: None,
                        page: 1,
                        printed_page: None,
                        label: None,
                        text: String::new(),
                        image: None,
                        piece: None,
                        caption: None,
                        name: None,
                    }],
                    ..SearchReply::default()
                }),
            },
        );
        deliver(
            &mut shared,
            Event::Answer {
                request: ask,
                result: Err(down(FailureKind::ClaudeUsageLimit)),
            },
        );
        assert!(
            matches!(shared.health.of(Service::Claude), ServiceState::Down(failure) if failure.kind == FailureKind::ClaudeUsageLimit)
        );
        assert_eq!(shared.health.level(), HealthLevel::Limited);

        // With ANTHROPIC_API_KEY set, Claude is down from the start and nothing changes that.
        let facts = StartupFacts {
            anthropic_api_key_set: true,
            ..StartupFacts::default()
        };
        let mut keyed = Shared::new(facts);
        let key_down = |shared: &Shared| matches!(shared.health.of(Service::Claude), ServiceState::Down(failure) if failure.kind == FailureKind::ClaudeApiKeySet);
        assert!(key_down(&keyed));
        let check = checking(&run(&mut keyed, Intent::RecheckHealth));
        assert!(key_down(&keyed), "a check never sets Claude to Checking");
        deliver(
            &mut keyed,
            Event::Health {
                request: check,
                service: Service::Claude,
                state: up(Service::Claude),
            },
        );
        assert!(key_down(&keyed), "an answer for Claude changes nothing");
        assert!(keyed.health.pending.is_some());
        answer_all(&mut keyed, check, up);
        assert_eq!(
            keyed.health.pending, None,
            "the check ends when the other five answered"
        );
        assert_eq!(keyed.health.level(), HealthLevel::Limited);
        let ask = asked(&mut keyed);
        keyed.apply_event(
            Event::Search {
                request: ask,
                result: Err(down(FailureKind::ClaudeUsageLimit)),
            },
            &mut Vec::new(),
        );
        assert!(
            key_down(&keyed),
            "a limit message does not hide the key problem"
        );
    }
}
