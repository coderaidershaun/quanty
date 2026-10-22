//! Runs commands on the caller's runtime and reports when each task ends, including after a
//! panic or cancellation.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use tokio::runtime::{Builder, Handle, Runtime as TokioRuntime};
use tokio::task::AbortHandle;

use super::handler::{Handler, Reply, Stop};
use contract::{Command, Event, Failure, RequestId};

const WORKER_THREADS: usize = 4;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

/// What a cancel and the question "is the backend idle" must know of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskKind {
    Ingest,
    BalanceRead,
    Other,
}

struct Task {
    handle: AbortHandle,
    stop: Stop,
    kind: TaskKind,
    finished: Arc<AtomicBool>,
}

type Spawn = Box<dyn Fn(Command) -> Task + Send>;

pub struct Runtime<H: Handler> {
    runtime: Option<TokioRuntime>,
    spawn: Spawn,
    finished: Receiver<RequestId>,
    marker: std::marker::PhantomData<H>,
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

impl<H: Handler> Runtime<H> {
    /// Every event a task sends is followed by a call to `wake`, so the window draws it.
    ///
    /// # Errors
    /// Whatever the operating system says when the threads cannot be made.
    pub fn start_own_threads(
        handler: H,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("quanty-backend")
            .enable_all()
            .build()?;
        let mut backend = Self::on(runtime.handle().clone(), Arc::new(handler), wake);
        backend.runtime = Some(runtime);
        Ok(backend)
    }

    pub fn on(handle: Handle, handler: Arc<H>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (sender, events) = channel();
        let (completed, finished) = channel();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let spawn: Spawn = Box::new(move |command| {
            let (stop, stopped) = Stop::pair();
            let reply = Reply::new(sender.clone(), Arc::clone(&wake), stopped);
            let kind = match command {
                Command::Ingest { .. } => TaskKind::Ingest,
                Command::ReadBalances { .. } => TaskKind::BalanceRead,
                _ => TaskKind::Other,
            };
            let handler = Arc::clone(&handler);
            let watch_reply = reply.clone();
            let completed = completed.clone();
            let request = command.request();
            let finished_wake = Arc::clone(&wake);
            let watched = command.clone();
            let finished = Arc::new(AtomicBool::new(false));
            let task_finished = finished.clone();
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
                let delivered = completed.send(request).is_ok();
                task_finished.store(true, Ordering::Release);
                if delivered {
                    finished_wake();
                }
            });
            Task {
                handle: abort,
                stop,
                kind,
                finished,
            }
        });
        Self {
            runtime: None,
            spawn,
            events,
            tasks: HashMap::new(),
            finished,
            marker: std::marker::PhantomData,
        }
    }

    pub fn send(&mut self, command: Command) {
        self.tasks
            .retain(|_, task| !task.finished.load(Ordering::Acquire));
        match command {
            Command::Cancel(request) => {
                // The task stays in the map until it is finished, so its `Stop` is not dropped
                // before the abort has taken effect: a dropped `Stop` also ends `cancelled`.
                if let Some(task) = self.tasks.get(&request) {
                    if task.kind == TaskKind::Ingest {
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

    pub fn take_finished(&mut self) -> Vec<RequestId> {
        self.finished.try_iter().collect()
    }

    pub fn try_next(&mut self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    pub fn is_idle(&self) -> bool {
        self.tasks
            .values()
            .all(|task| task.finished.load(Ordering::Acquire))
    }

    /// A read of the balances may hang for as long as its limit, and nothing else waits for it,
    /// so a control that only needs the other work to be done asks this.
    pub fn is_idle_but_for_balance_reads(&self) -> bool {
        self.tasks
            .values()
            .all(|task| task.kind == TaskKind::BalanceRead || task.finished.load(Ordering::Acquire))
    }
}

impl<H: Handler> Drop for Runtime<H> {
    fn drop(&mut self) {
        for task in self.tasks.values() {
            task.handle.abort();
        }
        self.tasks.clear();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(SHUTDOWN_WAIT);
        }
    }
}
