//! Keeps the six service checks together so the app can explain what is ready and what is down.

use std::collections::BTreeMap;

use super::shared::Shared;
use contract::{
    Command, Effect, Failure, FailureKind, NoticeKind, RequestId, Service, ServiceState,
    StartupFacts,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Health {
    pub facts: StartupFacts,
    /// `Some` while a check is running.
    pub pending: Option<RequestId>,
    /// A service with no entry is `Unknown`.
    states: BTreeMap<Service, ServiceState>,
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
        // SMELL: Model changes do not refresh the Ingest agent health.
        if health.has_conflicting_agent_key() {
            health.states.insert(
                Service::Claude,
                ServiceState::Down(Failure::new(
                    FailureKind::ClaudeApiKeySet,
                    "the ANTHROPIC_API_KEY variable is set",
                )),
            );
        }
        health
    }

    pub fn of(&self, service: Service) -> &ServiceState {
        self.states.get(&service).unwrap_or(&ServiceState::Unknown)
    }

    /// `Down` when no ask can work: a store or the embedding key is down. `Limited` when some
    /// other service is down.
    pub fn level(&self) -> HealthLevel {
        let is_down = |service| matches!(self.of(service), ServiceState::Down(_));
        let is_unknown = |service| *self.of(service) == ServiceState::Unknown;
        if [Service::Qdrant, Service::FalkorDb, Service::EmbeddingKey]
            .into_iter()
            .any(is_down)
        {
            HealthLevel::Down
        } else if Service::ALL.into_iter().any(is_down) {
            HealthLevel::Limited
        } else if self.is_checking() {
            HealthLevel::Checking
        } else if Service::ALL.into_iter().any(is_unknown) {
            HealthLevel::Unknown
        } else {
            HealthLevel::Ready
        }
    }

    fn set(&mut self, service: Service, state: ServiceState) {
        if service == Service::Claude && self.has_conflicting_agent_key() {
            return;
        }
        self.states.insert(service, state);
    }

    fn has_conflicting_agent_key(&self) -> bool {
        self.facts.anthropic_api_key_set && self.facts.model_choices.ingest.provider == "claude"
    }

    fn is_checking(&self) -> bool {
        self.states
            .values()
            .any(|state| *state == ServiceState::Checking)
    }
}

impl Shared {
    pub(super) fn recheck_health(&mut self, effects: &mut Vec<Effect>) {
        let request = self.restart(self.health.pending, effects);
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
        if !self.health.is_checking() {
            self.health.pending = None;
        }
    }

    pub(super) fn mark_down(&mut self, failure: &Failure) {
        if let Some(service) = failure.service() {
            self.health
                .set(service, ServiceState::Down(failure.clone()));
        }
    }

    /// Marks the service of the failure down, and tells the person with its hint.
    pub(super) fn fail_notice(&mut self, title: String, failure: &Failure) {
        self.mark_down(failure);
        self.add_notice(NoticeKind::Failed, title, failure.hint.clone());
    }
}
