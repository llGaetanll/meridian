//! Where a route's question is paired with its answer.
//!
//! The only door between a handler and the machine. A route hands over an
//! event and parks; the drain loop runs the machine and brings back the one
//! action owed to it — never another route's, which is the whole reason a
//! [`RequestId`] exists.
//!
//! It owns the pairing and nothing else: no decisions, no IO. Events go in
//! from every direction — routes, sources, timers, signals — and come out in
//! one batch, which is what makes a turn a batch rather than a single event.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use tokio::sync::oneshot;

use crate::ActionRouter;
use crate::Client;
use crate::Event;
use crate::EventRouter;
use crate::RequestId;

#[derive(Debug, Default)]
pub(crate) struct RouterExchange {
    /// Where [`RequestId`]s come from. At the edge, like the clock and the
    /// seed — the machine only relays them, so no decision depends on which
    /// number a request happened to draw.
    requests: AtomicU64,

    /// Everything waiting for the next turn.
    ///
    /// A plain `Vec` under a lock rather than a channel: the loop wants *all*
    /// of it at once, and taking a whole batch is one swap here against a
    /// drain loop on the other side.
    queue: Mutex<Vec<Event>>,

    /// The routes currently parked, by the request each is waiting on.
    ///
    /// A `oneshot` because a route is owed exactly one answer, and the type
    /// says so: the sender is consumed by sending, so a second answer to the
    /// same request cannot compile.
    parked: Mutex<HashMap<RequestId, oneshot::Sender<ActionRouter>>>,
}

impl RouterExchange {
    /// Ask the machine something on a route's behalf, and wait for its answer.
    ///
    /// The route parks rather than spins: `rx.await` yields the task, so a
    /// request in flight costs a map entry and nothing else.
    ///
    /// `None` means the answer never came. That is a bug rather than an
    /// outcome — every router event owes exactly one action back — and until
    /// there is a clock in the machine to expire an outstanding call, the only
    /// way to see one is a handler that never returns.
    pub(crate) async fn request(&self, client: Client, event: EventRouter) -> Option<ActionRouter> {
        let request_id = self.requests.fetch_add(1, Ordering::Relaxed);

        let (sender, receiver) = oneshot::channel();

        // Parked *before* the event is queued. The other order leaves a window
        // in which the loop answers a request nothing is waiting on yet, and
        // the answer is dropped.
        self.parked().insert(request_id, sender);

        self.push(Event::FromRouter {
            request_id,
            client,
            event,
        });

        match receiver.await {
            Ok(action) => Some(action),

            Err(_) => {
                self.parked().remove(&request_id);

                tracing::error!(request_id, "a route asked something nothing answered");

                None
            }
        }
    }

    /// Queue one event for the next turn.
    pub(crate) fn push(&self, event: Event) {
        self.queue().push(event);
    }

    /// Queue what carrying an action out produced.
    pub(crate) fn push_all(&self, events: Vec<Event>) {
        self.queue().extend(events);
    }

    /// Everything queued since the last turn, leaving the queue empty.
    pub(crate) fn drain(&self) -> Vec<Event> {
        std::mem::take(&mut *self.queue())
    }

    /// Hand one answer to the route parked on it.
    ///
    /// A missing entry is a route that hung up — a dropped connection — which
    /// is ordinary and not worth more than a line.
    pub(crate) fn deliver(&self, request_id: RequestId, action: ActionRouter) {
        let Some(sender) = self.parked().remove(&request_id) else {
            tracing::debug!(request_id, "answered a route that is no longer waiting");

            return;
        };

        if sender.send(action).is_err() {
            tracing::debug!(request_id, "a route hung up before its answer arrived");
        }
    }

    fn queue(&self) -> MutexGuard<'_, Vec<Event>> {
        Self::held(self.queue.lock(), "the event queue")
    }

    fn parked(&self) -> MutexGuard<'_, HashMap<RequestId, oneshot::Sender<ActionRouter>>> {
        Self::held(self.parked.lock(), "the parked routes")
    }

    /// A guard either way. A poisoned lock means another thread panicked while
    /// holding it; the data behind it is a queue and a map of channels, and
    /// carrying on with them is better than taking the server down.
    fn held<'a, T>(
        locked: Result<MutexGuard<'a, T>, std::sync::PoisonError<MutexGuard<'a, T>>>,
        what: &str,
    ) -> MutexGuard<'a, T> {
        match locked {
            Ok(guard) => guard,

            Err(poisoned) => {
                tracing::error!(
                    lock = what,
                    "lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held data",
                );

                poisoned.into_inner()
            }
        }
    }
}
