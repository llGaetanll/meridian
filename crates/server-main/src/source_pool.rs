//! The one place a call to a source is actually made.
//!
//! **Nothing here decides anything** — the pool owns only *how many at once*.
//! One per source, so a slow bank cannot use up the room a sign-in email
//! needs.

use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::Event;

/// A call waiting to be made, and its answer as events.
type Call = std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Event>> + Send>>;

pub(crate) struct SourcePool {
    /// Which source this is, for the log.
    name: &'static str,

    /// Calls go in here; the pool's own task takes them out.
    ///
    /// Unbounded, deliberately: a bounded channel would make handing over a
    /// call something that can block, putting the wait back in the loop this
    /// exists to keep clear. The semaphore bounds the work instead.
    calls: mpsc::UnboundedSender<Call>,
}

impl SourcePool {
    /// Start a pool and the task that runs it.
    ///
    /// `at_once` is how many calls may be in flight to this source. Every call
    /// handed over must eventually run: a route parks until its answer
    /// arrives, so one dropped is a request that never returns.
    pub(crate) fn new(
        name: &'static str,
        at_once: usize,
        landed: mpsc::UnboundedSender<Event>,
    ) -> Self {
        let (calls, mut waiting) = mpsc::unbounded_channel::<Call>();

        tokio::spawn(async move {
            let room = Arc::new(Semaphore::new(at_once));
            let mut running = JoinSet::new();

            loop {
                tokio::select! {
                    // Spawned rather than awaited: the point of this task is
                    // that it is never inside any one call.
                    call = waiting.recv() => {
                        let Some(call) = call else {
                            return;
                        };

                        let room = Arc::clone(&room);

                        running.spawn(async move {
                            // Dropped with the task, so the next call starts
                            // the instant this one finishes.
                            let _room = room.acquire_owned().await;

                            call.await
                        });
                    }

                    // One landed. Whatever it produced goes back to the loop.
                    Some(done) = running.join_next(), if !running.is_empty() => {
                        match done {
                            Ok(events) => {
                                for event in events {
                                    if landed.send(event).is_err() {
                                        return;
                                    }
                                }
                            }

                            // The call panicked. Whatever was waiting on its
                            // answer still is, and this is the only trace why.
                            Err(e) => tracing::error!(
                                source = name,
                                error = %e,
                                "a call to a source did not finish",
                            ),
                        }
                    }
                }
            }
        });

        Self { name, calls }
    }

    /// Hand a call over. Returns as soon as it is queued, never when it lands.
    pub(crate) fn call<C>(&self, call: C)
    where
        C: std::future::Future<Output = Vec<Event>> + Send + 'static,
    {
        if self.calls.send(Box::pin(call)).is_err() {
            tracing::error!(
                source = self.name,
                "a source pool is gone; its call was dropped",
            );
        }
    }
}
