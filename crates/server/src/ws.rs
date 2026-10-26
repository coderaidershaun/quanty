//! Each socket owns ordinary requests while the shared guard owns paid work.

use crate::jobs::{Guard, Owner, spawn_runtime};
use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    response::Response,
};
use backend::{Handler, Runtime};
use contract::{Command, Frame};
use std::sync::Arc;
use tokio::sync::mpsc;

pub(crate) struct SocketState<H> {
    pub(crate) handler: Arc<H>,
    pub(crate) guard: Guard,
}
impl<H> Clone for SocketState<H> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            guard: self.guard.clone(),
        }
    }
}

pub(crate) async fn upgrade<H: Handler>(
    State(state): State<SocketState<H>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade.on_upgrade(move |socket| connection(socket, state))
}

async fn send(socket: &mut WebSocket, frame: Frame) -> Result<(), ()> {
    let json = serde_json::to_string(&frame).map_err(|error| {
        tracing::error!(%error, "could not encode a backend frame");
    })?;
    socket
        .send(Message::Text(json.into()))
        .await
        .map_err(|_| ())
}

async fn flush<H: Handler>(socket: &mut WebSocket, runtime: &mut Runtime<H>) -> Result<(), ()> {
    // Completion can arrive while events are drained, so collect it first.
    let finished = runtime.take_finished();
    while let Some(event) = runtime.try_next() {
        send(socket, Frame::Event(event)).await?;
    }
    for request in finished {
        send(socket, Frame::Finished(request)).await?;
    }
    Ok(())
}

async fn connection<H: Handler>(mut socket: WebSocket, state: SocketState<H>) {
    let (mut runtime, wake) = spawn_runtime(state.handler.clone());
    let (frames, mut incoming) = mpsc::unbounded_channel();
    let owner = Owner::new(frames);
    loop {
        tokio::select! {
            message = socket.recv() => {
                let command = match message {
                    Some(Ok(Message::Text(json))) => serde_json::from_str::<Command>(&json).ok(),
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => None,
                };
                let Some(command) = command else {
                    if socket.send(Message::Close(Some(CloseFrame { code: 1007, reason: "bad command".into() }))).await.is_err() { tracing::debug!("bad-command socket already closed"); }
                    break;
                };
                match command {
                    Command::Cancel(request) => { if !state.guard.cancel(&owner, request) { runtime.send(Command::Cancel(request)); } },
                    command @ (Command::Ingest { .. } | Command::BuildDataset { .. }) => {
                        if let Err(failure) = state.guard.start(owner.clone(), command.clone(), state.handler.clone()) {
                            for event in command.failed(&failure) { if send(&mut socket, Frame::Event(event)).await.is_err() { return; } }
                            if send(&mut socket, Frame::Finished(command.request())).await.is_err() { return; }
                        }
                    },
                    command => runtime.send(command),
                }
            },
            _ = wake.notified() => {
                if flush(&mut socket, &mut runtime).await.is_err() { return; }
            },
            Some(frame) = incoming.recv() => { if send(&mut socket, frame).await.is_err() { return; } },
        }
    }
}
