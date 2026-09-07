//! What each Plaid Link session came to, and who is waiting to hear it.
//!
//! A sub-machine rather than fields on [`StateServer`](crate::StateServer):
//! nothing outside `/link/finish` and Plaid's `SESSION_FINISHED` reads any of
//! it. What it is for is the race between those two, which has no ordering:
//! either half can arrive first, so both are held until the other turns up.

use std::collections::HashMap;

use api_common::Timestamp;

use crate::ActionRouter;
use crate::PlaidCollect;
use crate::RequestId;

/// How long `/link/finish` waits for Plaid to say what a session came to.
///
/// The redirect that closes the sheet and Plaid's POST have no ordering
/// between them, so the route waits.
pub const LINK_ANSWER_GOOD_FOR_SECS: u64 = 10;

/// How long an outcome nobody has asked about is kept. A phone that has not
/// asked within ten minutes is not going to: the sheet is long closed.
pub const OUTCOME_GOOD_FOR_SECS: u64 = 10 * 60;

/// What a Link session came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkOutcome {
    Linked,

    /// The sheet was closed, or the exchange failed. The common ending.
    Nothing,
}

impl LinkOutcome {
    pub(crate) fn answer(self) -> ActionRouter {
        match self {
            LinkOutcome::Linked => ActionRouter::Linked { count: 1 },
            LinkOutcome::Nothing => ActionRouter::LinkAbandoned,
        }
    }
}

/// An outcome nobody has asked about yet.
#[derive(Debug)]
struct Settled {
    outcome: LinkOutcome,

    /// When it was recorded, so one nobody collects does not sit here.
    at: Timestamp,
}

/// A route parked on a session Plaid has not reported on yet.
#[derive(Debug)]
struct Watching {
    request_id: RequestId,

    /// When it parked, so it can be let go rather than held.
    since: Timestamp,
}

/// What a sheet closing came to, on a server Plaid calls.
#[derive(Debug)]
pub(crate) enum Closed {
    /// Plaid has already said, and the session is spent by the reading.
    Came {
        link_token: String,
        outcome: LinkOutcome,
    },

    /// Nothing open and nothing recorded, so there is nothing to wait for.
    Nothing,

    /// Parked on every session this caller has open. Any route displaced from
    /// one of them is owed an answer of its own, in this order.
    Parked { displaced: Vec<RequestId> },
}

/// The Link sessions in flight: what has been reported about them, and which
/// route is parked on which.
#[derive(Debug, Default)]
pub struct StateLink {
    /// Which channel collects the result of a Link session, read out of the
    /// environment on [`EventProcess::Started`](crate::EventProcess). `None`
    /// is a machine that has not been told the process is up.
    collect: Option<PlaidCollect>,

    /// What each Link session came to, for a route that has not asked yet,
    /// keyed by the `link_token` that `SESSION_FINISHED` carries. Always empty
    /// under `PLAID_COLLECT=poll`, where the close causes the collection.
    outcomes: HashMap<String, Settled>,

    /// The other order: a route that asked before Plaid said anything.
    /// [`outcomes`](Self::outcomes) is the half that arrived first.
    watching: HashMap<String, Watching>,
}

impl StateLink {
    /// What the process came up with.
    pub(crate) fn started(&mut self, collect: PlaidCollect) {
        self.collect = Some(collect);
    }

    /// Whether Plaid calls us with the result of a Link session.
    pub(crate) fn collects_by_webhook(&self) -> bool {
        self.collect == Some(PlaidCollect::Webhook)
    }

    /// A sheet closed, on a server Plaid calls.
    ///
    /// The recorded outcome is looked for before anybody is parked, because
    /// the POST usually wins the race and waiting first would make the route
    /// sit until it timed out.
    pub(crate) fn closed(
        &mut self,
        request_id: RequestId,
        pending: &[String],
        now: Timestamp,
    ) -> Closed {
        if let Some((link_token, outcome)) = self.collected(pending) {
            return Closed::Came {
                link_token,
                outcome,
            };
        }

        // Nothing open and nothing recorded: parking would promise nothing.
        if pending.is_empty() {
            return Closed::Nothing;
        }

        let mut displaced = Vec::new();

        // Parked on every session the caller has open. The first to report
        // answers, and the timeout is what stops this being forever.
        for link_token in pending {
            let was = self.watching.insert(
                link_token.clone(),
                Watching {
                    request_id,
                    since: now,
                },
            );

            // Two calls about the same session: the older is let go rather
            // than parked on a key it does not own, which nothing answers.
            if let Some(was) = was
                && was.request_id != request_id
            {
                displaced.push(was.request_id);
            }
        }

        Closed::Parked { displaced }
    }

    /// The recorded outcome for one of these sessions, taken as it is read. A
    /// bank attached wins over one that was not: somebody who backed out of
    /// their first bank and connected a second has two sessions.
    fn collected(&mut self, pending: &[String]) -> Option<(String, LinkOutcome)> {
        let found = pending
            .iter()
            .filter_map(|link_token| Some((link_token, self.outcomes.get(link_token)?.outcome)))
            .max_by_key(|(_, outcome)| *outcome == LinkOutcome::Linked)
            .map(|(link_token, outcome)| (link_token.clone(), outcome))?;

        self.outcomes.remove(&found.0);

        Some(found)
    }

    /// One route has been told what a session came to. Whatever else it was
    /// parked on goes with it: an entry left behind sends the next settlement
    /// to a dead request.
    pub(crate) fn reported(&mut self, request_id: RequestId) {
        self.watching
            .retain(|_, watching| watching.request_id != request_id);
    }

    /// One Link session has come to something. The route to tell, if one is
    /// parked on it; otherwise it is kept for the one that will ask.
    pub(crate) fn settled(
        &mut self,
        link_token: &str,
        outcome: LinkOutcome,
        now: Timestamp,
    ) -> Option<RequestId> {
        // No token means `/link/start` failed before there was one.
        if link_token.is_empty() {
            return None;
        }

        let Some(watching) = self.watching.remove(link_token) else {
            self.outcomes
                .insert(link_token.to_string(), Settled { outcome, at: now });

            return None;
        };

        Some(watching.request_id)
    }

    /// The routes that have waited long enough for Plaid to report, and the
    /// outcomes nobody ever came back for, both let go of.
    ///
    /// One route parked on two sessions has waited once and is owed one
    /// answer, so the list it returns holds each of them once.
    pub(crate) fn timed_out(&mut self, now: Timestamp) -> Vec<RequestId> {
        self.outcomes
            .retain(|_, settled| now.since(settled.at).as_secs() < OUTCOME_GOOD_FOR_SECS);

        let mut waited_out: Vec<RequestId> = self
            .watching
            .values()
            .filter(|watching| now.since(watching.since).as_secs() >= LINK_ANSWER_GOOD_FOR_SECS)
            .map(|watching| watching.request_id)
            .collect();

        waited_out.sort_unstable();
        waited_out.dedup();

        for request_id in &waited_out {
            self.watching
                .retain(|_, watching| watching.request_id != *request_id);
        }

        waited_out
    }

    /// Whether any route is parked on a session. The machine cannot come to a
    /// stop while one is: it is owed an answer nothing else will send.
    pub(crate) fn parked(&self) -> bool {
        !self.watching.is_empty()
    }

    /// How many outcomes are being kept for a route that has not asked.
    pub(crate) fn outcomes(&self) -> usize {
        self.outcomes.len()
    }

    /// How many routes are parked on a session.
    pub(crate) fn watching(&self) -> usize {
        self.watching.len()
    }
}
