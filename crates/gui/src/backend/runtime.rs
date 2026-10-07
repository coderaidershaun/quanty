//! Runs a backend on its own threads: one task for each command, cancelled by abort, except
//! an ingest, which is only asked to stop.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use tokio::runtime::{Builder, Runtime};
use tokio::task::AbortHandle;

use super::handler::{Handler, Reply, Stop};
use crate::contract::{Command, Event, RequestId};

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

impl Backend {
    /// Starts the runtime. Every event a task sends is followed by a call to `wake`, so the
    /// window draws it.
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
            let task = handle.spawn(async move { handler.serve(command, reply).await });
            Task {
                handle: task.abort_handle(),
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

    /// Starts the work of a command. `Cancel` aborts a task, or asks an ingest to stop.
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

    /// True when no task is running.
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::contract::{AskDraft, IngestOutcome, IngestRequest};

    /// Waits for a cancel and says so. Left alone it waits for 30 seconds.
    struct WaitsForCancel;

    impl Handler for WaitsForCancel {
        async fn serve(&self, command: Command, reply: Reply) {
            let request = command.request();
            tokio::select! {
                () = reply.cancelled() => reply.send(Event::IngestFinished {
                    request,
                    result: Ok(IngestOutcome::Cancelled),
                }),
                () = tokio::time::sleep(Duration::from_secs(30)) => {}
            }
        }
    }

    fn next_within(backend: &mut Backend, wait: Duration) -> Option<Event> {
        let started = Instant::now();
        while started.elapsed() < wait {
            if let Some(event) = backend.try_next() {
                return Some(event);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        None
    }

    #[test]
    fn a_cancel_aborts_a_task_but_only_asks_an_ingest_to_stop() {
        let mut backend = Backend::start(WaitsForCancel, || {}).expect("the runtime starts");

        let ask = RequestId(1);
        backend.send(Command::Ask {
            request: ask,
            ask: AskDraft::default(),
        });
        backend.send(Command::Cancel(ask));
        assert!(
            next_within(&mut backend, Duration::from_millis(150)).is_none(),
            "an aborted task never sends"
        );
        assert!(backend.is_idle());

        let ingest = RequestId(2);
        backend.send(Command::Ingest {
            request: ingest,
            ingest: IngestRequest::default(),
        });
        assert!(!backend.is_idle());
        backend.send(Command::Cancel(ingest));
        let ended = next_within(&mut backend, Duration::from_secs(2));
        assert!(matches!(
            ended,
            Some(Event::IngestFinished { request, result: Ok(IngestOutcome::Cancelled) }) if request == ingest
        ));
    }
}
