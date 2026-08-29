//! The one place a call to a source is actually made.
//!
//! Every request to Plaid, to Resend, or to whatever comes next goes through
//! one of these. A call is handed over and the caller carries on; the pool
//! keeps every one it has been given in a single set and pushes each answer
//! back onto the exchange as an ordinary event the moment it lands.
//!
//! **Nothing here decides anything.** It does not know what it is calling, what
//! an answer means, or whether the call was a good idea — the machine settled
//! that before the action was ever produced, and rate limits live there for
//! the same reason (`docs/ARCHITECTURE.md`, rule 7). What the pool owns is
//! *how many at once*, which is mechanics.
//!
//! One per source, so a slow bank cannot use up the room a sign-in email
//! needs.

use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::Event;
use crate::router_exchange::RouterExchange;

/// A call waiting to be made, and its answer as events.
type Call = std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Event>> + Send>>;

pub(crate) struct SourcePool {
    /// Which source this is, for the log.
    name: &'static str,

    /// Calls go in here; the pool's own task takes them out.
    ///
    /// Unbounded, deliberately. A bounded channel would make handing over a
    /// call something that can block, which would put the wait back in the
    /// loop this exists to keep clear. What bounds the work is the semaphore
    /// below; what bounds the *queue* is the machine, which will not ask for a
    /// call it has no rate budget for.
    calls: mpsc::UnboundedSender<Call>,
}

impl SourcePool {
    /// Start a pool and the task that runs it.
    ///
    /// `at_once` is how many calls may be in flight to this source. It replaced
    /// a `try_lock` that let one fetch through and silently dropped the rest —
    /// which, now that a route parks until its answer arrives, would be a
    /// request that never returns.
    pub(crate) fn new(name: &'static str, at_once: usize, exchange: Arc<RouterExchange>) -> Self {
        let (calls, mut waiting) = mpsc::unbounded_channel::<Call>();

        tokio::spawn(async move {
            let room = Arc::new(Semaphore::new(at_once));
            let mut running = JoinSet::new();

            loop {
                tokio::select! {
                    // A new call. Spawned rather than awaited: the point of
                    // this task is that it is never inside any one of them.
                    call = waiting.recv() => {
                        let Some(call) = call else {
                            return;
                        };

                        let room = Arc::clone(&room);

                        running.spawn(async move {
                            // Held for the call and dropped with the task, so
                            // the next one starts the instant this finishes.
                            let _room = room.acquire_owned().await;

                            call.await
                        });
                    }

                    // One landed. Whatever it produced goes back on the queue
                    // and the machine decides what it meant.
                    Some(done) = running.join_next(), if !running.is_empty() => {
                        match done {
                            Ok(events) => exchange.push_all(events),

                            // The call panicked. Reported rather than ignored:
                            // whatever was waiting on its answer is still
                            // waiting, and this is the only trace of why.
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
