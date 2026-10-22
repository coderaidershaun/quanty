//! A browser or an in-process caller delivers the same commands and events to the app.
use contract::{Command, Connection, Event};

pub trait Transport {
    fn send(&mut self, command: Command);
    fn try_next(&mut self) -> Option<Event>;
    fn is_idle(&self) -> bool;
    fn is_idle_but_for_balance_reads(&self) -> bool;
    fn connection(&self) -> Connection;
}
