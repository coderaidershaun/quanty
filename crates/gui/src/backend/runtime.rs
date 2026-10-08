//! Runs a backend on its own threads: one task for each command, cancelled by abort, except
//! an ingest, which is only asked to stop. A task that panics is answered for with a failure.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use tokio::runtime::{Builder, Runtime};
use tokio::task::AbortHandle;

use super::handler::{Handler, Reply, Stop};
use crate::contract::{Command, Event, Failure, RequestId};

const WORKER_THREADS: usize = 4;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

struct Task {
    handle: AbortHandle,
    stop: Stop,
    is_ingest: bool,
}

type Spawn = Box<dyn Fn(Command) -> Task>;

pub struct Backend {
    runtime: Option<Runtime>,
    spawn: Spawn,
    events: Receiver<Event>,
    tasks: HashMap<RequestId, Task>,
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    let message = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "no message".to_owned());
    format!("a backend task stopped on a bug: {message}")
}

impl Backend {
    /// Every event a task sends is followed by a call to `wake`, so the window draws it.
    ///
    /// # Errors
    /// Whatever the operating system says when the threads cannot be made.
    pub fn start(
        handler: impl Handler,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Backend> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("quanty-backend")
            .enable_all()
            .build()?;
        let (sender, events) = channel();
        let handler = Arc::new(handler);
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let handle = runtime.handle().clone();
        let spawn: Spawn = Box::new(move |command| {
            let (stop, stopped) = Stop::pair();
            let reply = Reply::new(sender.clone(), Arc::clone(&wake), stopped);
            let is_ingest = matches!(command, Command::Ingest { .. });
            let handler = Arc::clone(&handler);
            let watch_reply = reply.clone();
            let watched = command.clone();
            let task = handle.spawn(async move { handler.serve(command, reply).await });
            let abort = task.abort_handle();
            // A task that panics sends nothing, and the window would wait for it for ever. This
            // second task answers for it with a failure.
            handle.spawn(async move {
                if let Err(ended) = task.await
                    && ended.is_panic()
                {
                    let failure = Failure::internal(panic_text(ended.into_panic().as_ref()));
                    for event in watched.failed(&failure) {
                        watch_reply.send(event);
                    }
                }
            });
            Task {
                handle: abort,
                stop,
                is_ingest,
            }
        });
        Ok(Backend {
            runtime: Some(runtime),
            spawn,
            events,
            tasks: HashMap::new(),
        })
    }

    pub fn send(&mut self, command: Command) {
        self.tasks.retain(|_, task| !task.handle.is_finished());
        match command {
            Command::Cancel(request) => {
                // The task stays in the map until it is finished, so its `Stop` is not dropped
                // before the abort has taken effect: a dropped `Stop` also ends `cancelled`.
                if let Some(task) = self.tasks.get(&request) {
                    if task.is_ingest {
                        task.stop.ask();
                    } else {
                        task.handle.abort();
                    }
                }
            }
            command => {
                let request = command.request();
                let task = (self.spawn)(command);
                self.tasks.insert(request, task);
            }
        }
    }

    pub fn try_next(&mut self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    pub fn is_idle(&self) -> bool {
        self.tasks.values().all(|task| task.handle.is_finished())
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        // Dropping the stop senders tells a running ingest to stop before the shutdown wait.
        self.tasks.clear();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(SHUTDOWN_WAIT);
        }
    }
}
