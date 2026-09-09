//! The process did something to itself.
//!
//! Everything the machine cannot go and get: that it started, the entropy it
//! draws credentials from, and the time. All three arrive as facts from the
//! edge, which is what makes a log of the events a recording of the session.

use api_common::Timestamp;

use crate::PlaidCollect;

#[derive(Debug)]
pub enum EventProcess {
    /// The server came up, and this is what it came up with.
    ///
    /// `collect` is a fact the runtime observed off the environment, not a
    /// decision: a runtime that inspects its own configuration to decide what
    /// happens has behaviour no `eval!` test can reach.
    Started {
        collect: PlaidCollect,

        /// What webhooks from Stripe are checked against, where the process
        /// was configured for it. `None` refuses every one of them, which is
        /// the safe direction: an unverified webhook is somebody claiming a
        /// subscription was paid for.
        stripe_webhook_secret: Option<String>,
    },

    /// Fresh entropy for the machine's own generator.
    ///
    /// Randomness arrives the way time does: from the edge, as a fact, so that
    /// nothing inside reads a source of its own. Reseeding on a timer bounds
    /// the damage of a leaked log — a seed only tells you the codes drawn
    /// before the next one.
    Seeded { seed: [u8; 32] },

    /// What time it is now.
    ///
    /// Also what lets the machine notice that *nothing* happened: a call that
    /// never answers produces no event, so without a tick there is no turn in
    /// which to give up on it.
    Tick { now: Timestamp },
}
