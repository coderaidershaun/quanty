//! What is known about the six services, and the one word that sums it up.

use std::collections::BTreeMap;

use super::shared::{Shared, push_cancel};
use crate::contract::{
    Command, Effect, Failure, FailureKind, RequestId, Service, ServiceState, StartupFacts,
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
        if health.facts.anthropic_api_key_set {
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

    /// While `ANTHROPIC_API_KEY` is set, nothing changes the state of Claude.
    fn set(&mut self, service: Service, state: ServiceState) {
        if service == Service::Claude && self.facts.anthropic_api_key_set {
            return;
        }
        self.states.insert(service, state);
    }

    fn is_checking(&self) -> bool {
        self.states
            .values()
            .any(|state| *state == ServiceState::Checking)
    }
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
}
