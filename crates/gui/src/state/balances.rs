//! What the footer shows of each paid account: what the last read found, and which providers a
//! paid reply said are out of credit.

use std::collections::BTreeSet;

use super::shared::{Shared, push_cancel};
use contract::{
    Balance, BalanceReads, Command, Effect, Event, FailureKind, IngestOutcome, Provider, RequestId,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Balances {
    /// `Some` while a read is on its way.
    pending: Option<RequestId>,
    /// What the last read found. `None` until the first read ends.
    found: Option<BalanceReads>,
    /// Providers that a paid reply said are out of credit, and that no job needing them has
    /// worked for since.
    out_of_credit: BTreeSet<Provider>,
}

impl Balances {
    /// What the footer shows for `provider`. A key problem found by the last read comes first,
    /// because no sighting of credit matters while the key does not work. `None` until the first
    /// read ends and no paid reply has said anything.
    pub fn shown(&self, provider: Provider) -> Option<Balance> {
        let read = self.found.map(|found| found.of(provider));
        match (read, self.out_of_credit.contains(&provider)) {
            (Some(problem @ (Balance::KeyMissing | Balance::KeyRefused)), _) => Some(problem),
            (_, true) => Some(Balance::OutOfCredit),
            (read, false) => read,
        }
    }

    pub fn is_reading(&self) -> bool {
        self.pending.is_some()
    }
}

impl Shared {
    pub(super) fn read_balances(&mut self, effects: &mut Vec<Effect>) {
        if let Some(old) = self.balances.pending {
            push_cancel(effects, old);
        }
        let request = self.issue_request();
        self.balances.pending = Some(request);
        effects.push(Effect::Send(Command::ReadBalances { request }));
    }

    pub(super) fn balances_arrived(&mut self, request: RequestId, found: BalanceReads) {
        if self.balances.pending != Some(request) {
            return;
        }
        self.balances.found = Some(found);
        self.balances.pending = None;
    }

    /// Notes and forgets spent credit from the event itself, whichever request it answers: any
    /// reply from a provider tells something about its credit, even one that came late.
    pub(super) fn hear_paid_reply(&mut self, event: &Event) {
        let out_of_credit = &mut self.balances.out_of_credit;
        match event {
            Event::Search {
                result: Err(failure),
                ..
            }
            | Event::IngestFinished {
                result: Err(failure),
                ..
            } => match failure.kind {
                FailureKind::EmbeddingOutOfCredit => {
                    out_of_credit.insert(Provider::Google);
                }
                FailureKind::ConverterOutOfCredit => {
                    out_of_credit.insert(Provider::Jev);
                }
                _ => {}
            },
            // A search with no document to look at stops before it embeds the question, so it
            // says nothing of Google.
            Event::Search {
                result: Ok(reply), ..
            } if reply.trace.documents_searched != Some(0) => {
                out_of_credit.remove(&Provider::Google);
            }
            // SMELL: a run with no page left to convert never calls Jev, yet it clears the note
            // too. The footer then says Jev offers no balance, which claims nothing about credit.
            Event::IngestFinished {
                result: Ok(IngestOutcome::Ingested(_)),
                ..
            } => out_of_credit.clear(),
            _ => {}
        }
    }

    /// The ask, the ingest run and the dataset build: the jobs that spend credit.
    pub(super) fn paid_jobs(&self) -> [bool; 3] {
        [
            self.ask.is_running(),
            self.ingest.is_running(),
            self.datasets.is_running(),
        ]
    }

    /// Reads the balances again when a paid job that `before` shows running has stopped since.
    pub(super) fn read_after_a_paid_job(&mut self, before: [bool; 3], effects: &mut Vec<Effect>) {
        let has_stopped = before
            .into_iter()
            .zip(self.paid_jobs())
            .any(|(was_running, is_running)| was_running && !is_running);
        if has_stopped {
            self.read_balances(effects);
        }
    }
}
