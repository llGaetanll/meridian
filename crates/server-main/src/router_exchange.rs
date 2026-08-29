//! Where a route's question is paired with its answer.
//!
//! The only door between a handler and the machine. A route hands over an
//! event and gets back the one action owed to it — never another route's,
//! which is the whole reason a [`RequestId`] exists.
//!
//! It owns the pairing and nothing else: no decisions, no IO. Today the wait
//! is a dispatch that returns; when the drain loop lands, the queue and the
//! parked handlers move in here and the routes do not change.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::Action;
use crate::ActionRouter;
use crate::RequestId;

#[derive(Debug, Default)]
pub(crate) struct RouterExchange {
    /// Where [`RequestId`]s come from. At the edge, like the clock and the
    /// seed — the machine only relays them, so no decision depends on which
    /// number a request happened to draw.
    requests: AtomicU64,
}

impl RouterExchange {
    /// A fresh id, for a request about to be asked.
    pub(crate) fn opening(&self) -> RequestId {
        self.requests.fetch_add(1, Ordering::Relaxed)
    }

    /// The answer owed to one request, out of everything a turn produced.
    ///
    /// `None` means nothing answered it, which is a bug rather than an
    /// outcome: every router event owes exactly one action back.
    pub(crate) fn answer(
        &self,
        request_id: RequestId,
        actions: Vec<Action>,
    ) -> Option<ActionRouter> {
        let answer = actions.into_iter().find_map(|action| match action {
            Action::ToRouter {
                request_id: answered,
                action,
            } if answered == request_id => Some(action),

            _ => None,
        });

        if answer.is_none() {
            tracing::error!(request_id, "a route asked something nothing answered");
        }

        answer
    }
}
