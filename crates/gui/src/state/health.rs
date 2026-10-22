//! What is known about the six services, the one word that sums it up, and the database in use,
//! whose two stores are the first two services.

use std::collections::BTreeMap;

use choices::Chosen;

use super::shared::{Shared, push_cancel};
use crate::contract::{
    Command, Database, Effect, Failure, FailureKind, Loadable, RequestId, Service, ServiceState,
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
        // SMELL: Startup health assumes Claude even when Codex is selected; model changes do not refresh it.
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

/// The database that every command reads and writes, and how the checks of it ended.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DatabaseLink {
    /// `None` until one is chosen, and nothing loads until then.
    pub in_use: Option<Database>,
    /// The check on its way: its request and the database it checks.
    checking: Option<(RequestId, Database)>,
    /// How the last check of `in_use` ended. `Idle` until it is checked. The footer draws from it.
    pub state: Loadable<()>,
    /// How the last switch asked from the sheet ended, for the sheet's message. Cleared when the
    /// next one is sent.
    pub switch_failure: Option<Failure>,
}

impl DatabaseLink {
    /// The database that the saved choice names, or none when nothing is chosen or the choice
    /// cannot be understood.
    pub(super) fn chosen_by(facts: &StartupFacts) -> DatabaseLink {
        let in_use = match facts.databases.chosen() {
            Ok(Chosen::Local) => Some(facts.local_database.clone()),
            Ok(Chosen::Saved(database)) => Some(database.clone()),
            Ok(Chosen::Nothing) => None,
            Err(error) => {
                tracing::error!(%error, "the chosen database is not one of the saved ones");
                None
            }
        };
        DatabaseLink {
            in_use,
            ..DatabaseLink::default()
        }
    }

    pub fn is_checking(&self) -> bool {
        self.checking.is_some()
    }
}

impl Shared {
    /// A switch waits for any ask, ingest or build, which would lose its stores halfway, and for
    /// a check on its way, which a second check would cancel.
    pub fn can_switch_database(&self) -> bool {
        !self.ask.is_running()
            && !self.ingest.is_running()
            && !self.datasets.is_running()
            && !self.database.is_checking()
    }

    pub(super) fn use_database(&mut self, database: Database, effects: &mut Vec<Effect>) {
        let request = self.issue_request();
        if let Some((old, _)) = self.database.checking.take() {
            push_cancel(effects, old);
        }
        if self.database.in_use.as_ref() == Some(&database) {
            self.database.state = Loadable::Loading;
        } else if self.database.state.is_loading() {
            // The cancelled check was the one of the database in use, so it is not known now.
            self.database.state = Loadable::Idle;
        }
        self.database.switch_failure = None;
        self.database.checking = Some((request, database.clone()));
        effects.push(Effect::Send(Command::UseDatabase { request, database }));
    }

    /// A check of the database in use ends in `state` and in the health of its stores, and the
    /// catalogue is read either way, so that the Library says what it finds. A switch to another
    /// database changes nothing when it fails but the message of the sheet.
    pub(super) fn database_used(
        &mut self,
        request: RequestId,
        result: Result<(), Failure>,
        effects: &mut Vec<Effect>,
    ) {
        let Some((_, database)) = self
            .database
            .checking
            .take_if(|(pending, _)| *pending == request)
        else {
            return;
        };
        let is_switch = self.database.in_use.as_ref() != Some(&database);
        match result {
            Ok(()) => {
                self.health.set(
                    Service::Qdrant,
                    ServiceState::Up {
                        detail: database.qdrant_url.clone(),
                    },
                );
                self.health.set(
                    Service::FalkorDb,
                    ServiceState::Up {
                        detail: database.falkordb_url.clone(),
                    },
                );
                self.database.in_use = Some(database);
                self.database.state = Loadable::Ready(());
                self.refresh_catalogue(effects);
            }
            Err(failure) if is_switch => self.database.switch_failure = Some(failure),
            Err(failure) => {
                self.mark_down(&failure);
                self.database.state = Loadable::Failed(failure);
                self.refresh_catalogue(effects);
            }
        }
    }

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
