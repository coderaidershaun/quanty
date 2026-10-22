//! Tests run a real backend without a socket.
use crate::transport::Transport;
use backend::{Handler, Runtime};
use contract::{Command, Connection, Event};

pub struct InProcess<H: Handler>(Runtime<H>);
impl<H: Handler> InProcess<H> {
    pub fn new(handler: H, wake: impl Fn() + Send + Sync + 'static) -> std::io::Result<Self> {
        Runtime::start_own_threads(handler, wake).map(Self)
    }
}
impl<H: Handler> Transport for InProcess<H> {
    fn send(&mut self, command: Command) {
        self.0.send(command);
    }
    fn try_next(&mut self) -> Option<Event> {
        self.0.try_next()
    }
    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }
    fn is_idle_but_for_balance_reads(&self) -> bool {
        self.0.is_idle_but_for_balance_reads()
    }
    fn connection(&self) -> Connection {
        Connection::Connected
    }
}

pub(super) struct Home<H> {
    pub handler: H,
    pub _folder: tempfile::TempDir,
}
impl<H: Handler> Handler for Home<H> {
    async fn serve(&self, command: Command, reply: backend::Reply) {
        self.handler.serve(command, reply).await;
    }
}
