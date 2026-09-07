//! What is out, who is waiting on it, and what they get when it lands.
//!
//! Every answer to a route goes through here, so no handler has to remember to
//! wait for the writes it started. Shared rather than owned by one machine:
//! signing up, a snapshot and a webhook all make calls, and they draw their ids
//! from one counter so that the same event log draws the same numbers.

use std::collections::HashMap;

use api_common::Timestamp;

use crate::Action;
use crate::ActionRouter;
use crate::ActionStore;
use crate::CallId;
use crate::Owed;
use crate::Pending;
use crate::RequestId;
use crate::Waiting;

/// How long to wait on a source before giving up on it. Not a latency budget
/// but the point past which a call has almost certainly gone quiet.
pub const CALL_GOOD_FOR_SECS: u64 = 30;

/// The calls in flight and the routes parked on them.
#[derive(Debug, Default)]
pub struct Calls {
    /// The next [`CallId`] to hand out. A counter rather than anything drawn,
    /// so the same log replays to the same numbers.
    next: CallId,

    /// Which route, if any, is parked on the answer to each call in flight.
    /// An absent entry is a call nobody is waiting on — a sync on a timer.
    waiting: HashMap<CallId, Waiting>,

    /// Answers that are decided but not yet owed. A route reads the store the
    /// moment it is told to, so one released while a write is in flight serves
    /// the rows that write was about to replace.
    held: HashMap<RequestId, Owed>,

    /// Requests waiting on a read, by the call that asked for it: what a
    /// request was has to outlive the turn it arrived in.
    pending: HashMap<CallId, Pending>,
}

impl Calls {
    /// A fresh [`CallId`], noted as owed to the route that is waiting.
    pub fn calling(&mut self, request_id: RequestId, now: Timestamp) -> CallId {
        let call_id = self.call();

        self.waiting.insert(
            call_id,
            Waiting {
                request_id,
                since: now,
            },
        );

        call_id
    }

    /// A fresh [`CallId`] for a call nobody is waiting on.
    pub fn call(&mut self) -> CallId {
        self.next += 1;

        self.next
    }

    /// A fresh [`CallId`] owed to the same route as `call_id`, which keeps its
    /// own wait. For an answer that causes two: this is the branch that does
    /// not carry the chain on, and the other takes the wait with
    /// [`passing`](Self::passing).
    pub fn alongside(&mut self, call_id: CallId, now: Timestamp) -> CallId {
        match self.waiting.get(&call_id) {
            Some(waiter) => {
                let request_id = waiter.request_id;

                self.calling(request_id, now)
            }
            None => self.call(),
        }
    }

    /// A fresh [`CallId`] that inherits whatever `call_id` was holding. One
    /// call causes the next, and the route waits for the end of that chain.
    pub fn passing(&mut self, call_id: CallId, now: Timestamp) -> CallId {
        match self.waiting.remove(&call_id) {
            Some(waiter) => self.calling(waiter.request_id, now),
            None => self.call(),
        }
    }

    /// Who was waiting on this call, now that it has answered. Absent means an
    /// answer to a question we have no record of asking, and picking a route
    /// for it would send somebody another person's bank.
    pub fn answered(&mut self, call_id: CallId) -> Option<RequestId> {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            tracing::error!(call_id, "a source answered a call nobody is waiting on");

            return None;
        };

        Some(request_id)
    }

    /// Note what a request will do with the answer to the read it just asked
    /// for.
    pub fn waits_on(&mut self, call_id: CallId, pending: Pending) {
        self.pending.insert(call_id, pending);
    }

    /// What the request waiting on this read was doing.
    pub fn resumed(&mut self, call_id: CallId) -> Option<Pending> {
        self.pending.remove(&call_id)
    }

    /// What a route is owed, sent the moment nothing it started is still out.
    pub fn answer(
        &mut self,
        request_id: RequestId,
        action: ActionRouter,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        self.owe(request_id, Owed::Answer(action), now, actions);
    }

    /// Hold what a request is owed until everything it started has landed.
    pub fn owe(
        &mut self,
        request_id: RequestId,
        owed: Owed,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        if self.outstanding(request_id) {
            self.held.insert(request_id, owed);

            return;
        }

        self.release(request_id, owed, now, actions);
    }

    /// Nothing is outstanding, so this is what the request gets. A snapshot is
    /// one more call rather than an answer: the rows it reads are the ones the
    /// calls that just landed wrote.
    fn release(
        &mut self,
        request_id: RequestId,
        owed: Owed,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) {
        match owed {
            Owed::Answer(action) => actions.push(Action::ToRouter { request_id, action }),

            Owed::Snapshot { user_id } => {
                let call_id = self.calling(request_id, now);

                actions.push(Action::ToStore {
                    action: ActionStore::LoadSnapshot { call_id, user_id },
                });
            }
        }
    }

    /// One call has landed. Release the answer it was the last thing holding.
    /// Silent when nothing was waiting, unlike [`answered`](Self::answered):
    /// that is the ordinary shape of a sync nobody asked for.
    pub fn landed(&mut self, call_id: CallId, now: Timestamp, actions: &mut Vec<Action>) {
        let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
            return;
        };

        // Somebody with three banks is answered after the third, not the first.
        if self.outstanding(request_id) {
            return;
        }

        let Some(owed) = self.held.remove(&request_id) else {
            return;
        };

        self.release(request_id, owed, now, actions);
    }

    /// Whether this route started anything that has not come back.
    pub fn outstanding(&self, request_id: RequestId) -> bool {
        self.waiting
            .values()
            .any(|waiter| waiter.request_id == request_id)
    }

    /// What is being held for this route, for an answer that adds to it.
    pub fn owed(&self, request_id: RequestId) -> Option<&Owed> {
        self.held.get(&request_id)
    }

    /// Stop waiting on calls that have taken too long.
    ///
    /// The reason there is a tick at all: a source that hangs rather than fails
    /// produces no event, and the route waiting on it would be parked forever.
    /// It answers with whatever was decided, and
    /// [`ActionRouter::Unavailable`] when nothing was.
    pub fn timed_out(&mut self, now: Timestamp, actions: &mut Vec<Action>) {
        let given_up: Vec<CallId> = self
            .waiting
            .iter()
            .filter(|(_, waiter)| now.since(waiter.since).as_secs() >= CALL_GOOD_FOR_SECS)
            .map(|(call_id, _)| *call_id)
            .collect();

        for call_id in given_up {
            let Some(Waiting { request_id, .. }) = self.waiting.remove(&call_id) else {
                unreachable!("the key came from the map")
            };

            tracing::warn!(
                call_id,
                request_id,
                "a call took too long and was given up on"
            );

            // What the request was doing goes with the call it was waiting on.
            // Nothing else ever removes this: an answer is what does, and
            // giving up is deciding there will not be one.
            self.pending.remove(&call_id);

            // Another of this route's calls may still answer and release it.
            if self.outstanding(request_id) {
                continue;
            }

            // A snapshot still reads and still serves: a call that never
            // answered costs the fresher rows, not the ledger already there.
            let owed = self
                .held
                .remove(&request_id)
                .unwrap_or(Owed::Answer(ActionRouter::Unavailable));

            self.release(request_id, owed, now, actions);
        }
    }

    /// Whether nothing is out, held or parked. The machine cannot come to a
    /// stop before this is true.
    pub fn quiet(&self) -> bool {
        self.waiting.is_empty() && self.held.is_empty() && self.pending.is_empty()
    }

    /// How many calls are out. See [`CensusServer`](crate::CensusServer).
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }

    /// How many answers are being held.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// How many requests are parked on a read.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}
