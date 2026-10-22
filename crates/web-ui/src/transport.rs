//! Socket completion and disconnection both release the work the app is waiting for.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Sender;
use std::time::Duration;

use contract::{Command, Connection, Event, Failure, Frame, Intent, RequestId};
use eframe::egui;
use ewebsock::{WsEvent, WsMessage, WsReceiver, WsSender};
use gui::transport::Transport;
use web_time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TaskKind {
    Ingest,
    BalanceRead,
    Other,
}

pub(crate) struct SocketTransport {
    url: String,
    socket: Option<(WsSender, WsReceiver)>,
    ctx: egui::Context,
    intents: Sender<Intent>,
    events: VecDeque<Event>,
    outstanding: HashMap<RequestId, TaskKind>,
    failures: HashMap<RequestId, Vec<Event>>,
    connection: Connection,
    retry_at: Option<Instant>,
    attempt: u32,
    has_connected: bool,
    startup_failed: bool,
}

impl SocketTransport {
    pub(crate) fn connect(
        path: &str,
        ctx: egui::Context,
        intents: Sender<Intent>,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        let location = web_sys::window()
            .expect("the browser provides a window")
            .location();
        let protocol = if location.protocol()? == "https:" {
            "wss"
        } else {
            "ws"
        };
        let mut transport = Self {
            url: format!("{protocol}://{}{path}", location.host()?),
            socket: None,
            ctx,
            intents,
            events: VecDeque::new(),
            outstanding: HashMap::new(),
            failures: HashMap::new(),
            connection: Connection::Reconnecting { attempt: 1 },
            retry_at: None,
            attempt: 0,
            has_connected: false,
            startup_failed: false,
        };
        transport.open();
        Ok(transport)
    }

    fn open(&mut self) {
        self.retry_at = None;
        let ctx = self.ctx.clone();
        match ewebsock::connect_with_wakeup(&self.url, ewebsock::Options::default(), move || {
            ctx.request_repaint();
        }) {
            Ok(socket) => self.socket = Some(socket),
            Err(error) => {
                log::warn!("could not open server socket: {error}");
                self.disconnect();
            }
        }
    }

    fn disconnect(&mut self) {
        self.socket = None;
        for (_, events) in self.failures.drain() {
            self.events.extend(events);
        }
        self.outstanding.clear();
        if self.retry_at.is_none() {
            let seconds = match self.attempt {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 8,
                _ => 15,
            };
            self.attempt = self.attempt.saturating_add(1);
            let delay = Duration::from_secs(seconds);
            self.retry_at = Some(Instant::now() + delay);
            self.connection = Connection::Reconnecting {
                attempt: self.attempt,
            };
            self.ctx.request_repaint_after(delay);
        }
        self.ctx.request_repaint();
    }

    fn refresh(&self, intent: Intent) {
        if self.intents.send(intent).is_ok() {
            self.ctx.request_repaint();
        }
    }

    fn poll(&mut self) {
        if let Some(retry_at) = self.retry_at {
            if Instant::now() >= retry_at {
                self.open();
            } else {
                self.ctx
                    .request_repaint_after(retry_at.saturating_duration_since(Instant::now()));
            }
        }
        loop {
            let event = self
                .socket
                .as_ref()
                .and_then(|(_, receiver)| receiver.try_recv());
            let Some(event) = event else { break };
            match event {
                WsEvent::Opened => {
                    self.connection = Connection::Connected;
                    self.attempt = 0;
                    if self.has_connected || self.startup_failed {
                        self.refresh(Intent::RecheckHealth);
                        self.refresh(Intent::RefreshCatalogue);
                    }
                    if self.startup_failed {
                        self.refresh(Intent::ReadBalances);
                    }
                    self.startup_failed = false;
                    self.has_connected = true;
                }
                WsEvent::Message(WsMessage::Text(text)) => {
                    match serde_json::from_str::<Frame>(&text) {
                        Ok(Frame::Event(event)) => self.events.push_back(event),
                        Ok(Frame::Finished(request)) => {
                            self.outstanding.remove(&request);
                            self.failures.remove(&request);
                        }
                        Err(error) => {
                            log::error!("invalid server frame: {error}");
                            self.disconnect();
                        }
                    }
                }
                WsEvent::Error(error) => {
                    log::warn!("server socket failed: {error}");
                    self.disconnect();
                }
                WsEvent::Closed => self.disconnect(),
                WsEvent::Message(_) => {
                    log::error!("the server sent a non-text frame");
                    self.disconnect();
                }
            }
        }
    }
}

impl Transport for SocketTransport {
    fn send(&mut self, command: Command) {
        self.poll();
        let failure = Failure::internal("not connected to the server");
        if self.connection != Connection::Connected {
            self.events.extend(command.failed(&failure));
            self.startup_failed |= !self.has_connected;
            self.ctx.request_repaint();
            return;
        }
        let text = match serde_json::to_string(&command) {
            Ok(text) => text,
            Err(error) => {
                self.events
                    .extend(command.failed(&Failure::internal(format!(
                        "could not encode server command: {error}"
                    ))));
                self.ctx.request_repaint();
                return;
            }
        };
        if !matches!(command, Command::Cancel(_)) {
            let kind = match command {
                Command::Ingest { .. } => TaskKind::Ingest,
                Command::ReadBalances { .. } => TaskKind::BalanceRead,
                _ => TaskKind::Other,
            };
            self.outstanding.insert(command.request(), kind);
            self.failures
                .insert(command.request(), command.failed(&failure));
        }
        if let Some((socket, _)) = &mut self.socket {
            socket.send(WsMessage::Text(text));
        }
    }

    fn try_next(&mut self) -> Option<Event> {
        self.poll();
        self.events.pop_front()
    }

    fn is_idle(&self) -> bool {
        self.outstanding.is_empty()
    }

    fn is_idle_but_for_balance_reads(&self) -> bool {
        self.outstanding
            .values()
            .all(|kind| *kind == TaskKind::BalanceRead)
    }

    fn connection(&self) -> Connection {
        self.connection
    }
}
