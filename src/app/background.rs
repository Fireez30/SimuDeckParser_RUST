//! Running slow work (network, disk, parsing) without freezing the window.
//!
//! The UI redraws many times per second, so it must never wait. Slow work is started with
//! [`Task::spawn`], which runs it on a new thread, and the screen that started it calls
//! [`Task::poll`] on each frame until the result arrives.

use std::sync::mpsc::{self, Receiver, TryRecvError};

use anyhow::{Result, anyhow};

/// Work running on a background thread, which ends with a `T` or an error.
pub struct Task<T> {
    /// The receiving end of a channel: the thread sends its result through it.
    result: Receiver<Result<T>>,
}

// `T: Send + 'static` says the result can be moved from the worker thread to the UI thread.
impl<T: Send + 'static> Task<T> {
    /// Runs `job` on a new thread. When it is done, the window is asked to redraw, so the
    /// screen polls the result right away.
    pub fn spawn(ctx: &egui::Context, job: impl FnOnce() -> Result<T> + Send + 'static) -> Self {
        let (sender, result) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // The screen may be gone by now (the receiver dropped): then nobody wants the
            // result, so a failed send is fine.
            let _ = sender.send(job());
            ctx.request_repaint();
        });
        Self { result }
    }

    /// `None` while the job runs; then its result. A job that crashed (panicked) ends with an
    /// error too. Prefer [`finished`], which also forgets the task once it is done.
    pub fn poll(&self) -> Option<Result<T>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(anyhow!("the background task stopped unexpectedly")))
            }
        }
    }
}

/// Checks on the task kept in `slot`, if there is one: once it is done, empties the slot and
/// returns the result. Called on every frame, typically as
/// `if let Some(result) = finished(&mut self.task) { … }`.
pub fn finished<T: Send + 'static>(slot: &mut Option<Task<T>>) -> Option<Result<T>> {
    let result = slot.as_ref()?.poll()?;
    *slot = None;
    Some(result)
}
