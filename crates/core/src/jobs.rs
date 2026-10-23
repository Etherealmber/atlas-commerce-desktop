//! Background work without ever blocking the egui UI thread.
//!
//! A multi-thread Tokio runtime owns all network work. Futures are spawned on
//! it and hand their result back through a standard library channel, so the
//! interface can poll a [`Job`] once per frame with no async in the UI layer.

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvError, TryRecvError};

use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot;

/// The process-wide runtime, built once before the window opens.
pub struct JobRunner {
    runtime: Runtime,
}

impl JobRunner {
    #[must_use]
    pub fn new() -> Self {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("atlas-worker")
            .enable_all()
            .build()
            .expect("the Tokio runtime builds");
        Self { runtime }
    }

    /// Spawns a future on the runtime and returns a pollable handle.
    pub fn spawn<F, T>(&self, future: F) -> Job<T>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let (cancel, _cancel_guard) = oneshot::channel::<()>();
        self.runtime.spawn(async move {
            // The channel only errors when the Job was dropped; then the
            // result has no reader and is discarded.
            let _ = tx.send(future.await);
            drop(cancel);
        });
        Job {
            receiver: Mutex::new(rx),
        }
    }

    /// Runs an async closure to completion on the runtime from a synchronous
    /// (worker) thread, for the self-update machinery which is blocking.
    pub fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

impl Default for JobRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// A background computation whose result the UI polls each frame.
pub struct Job<T> {
    receiver: Mutex<Receiver<T>>,
}

impl<T> Job<T> {
    /// Returns the result the first time it is ready, then keeps returning
    /// `None` (poll is idempotent after completion).
    #[must_use]
    pub fn poll(&self) -> Option<T> {
        match self.receiver.lock().ok()?.try_recv() {
            Ok(value) => Some(value),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Blocks the calling (non-UI) thread until the result arrives.
    ///
    /// # Errors
    /// Propagates the channel's [`RecvError`] if the task vanished.
    pub fn wait(&self) -> Result<T, RecvError> {
        self.receiver
            .lock()
            .map_or(Err(RecvError), |receiver| receiver.recv())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn job_delivers_its_result() {
        let runner = JobRunner::new();
        let job = runner.spawn(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        });
        assert_eq!(job.wait().unwrap(), 42);
    }

    #[test]
    fn poll_is_empty_then_ready() {
        let runner = JobRunner::new();
        let job = runner.spawn(async { "done" });
        let mut answer = None;
        for _ in 0..100 {
            if let Some(value) = job.poll() {
                answer = Some(value);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(answer, Some("done"));
        // A second poll does not yield the value again.
        assert!(job.poll().is_none());
    }
}
