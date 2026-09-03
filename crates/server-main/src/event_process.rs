//! The process did something to itself.
//!
//! The counterpart of [`ActionProcess`](crate::ActionProcess): that one is
//! what the machine asks the process to do, this is what the process reports
//! having done. Distinct from [`EventTerminal`](crate::EventTerminal), which is
//! the process being *signalled* from outside.
//!
//! Everything the process hands the machine that the machine cannot go and
//! get: that it started, the entropy it draws credentials from, and the time.
//! All three arrive as facts from the edge, which is what makes a log of the
//! events a recording of the session.

use api_common::Timestamp;

use crate::PlaidCollect;

#[derive(Debug)]
pub enum EventProcess {
    /// The server came up, and this is what it came up with.
    ///
    /// `collect` is a *fact the runtime observed* — it read the environment
    /// and validated it — and not a decision. What to do about it is the
    /// machine's: with `Poll` a closed sheet is something to go and ask Plaid
    /// about, and with `Webhook` it is a read of what has already arrived.
    ///
    /// It is on the event rather than checked where the link flow needs it,
    /// because a runtime that inspects its own configuration to decide what
    /// happens is a runtime with behaviour in it — behaviour no `eval!` test
    /// can reach and no event log records.
    Started { collect: PlaidCollect },

    /// Fresh entropy for the machine's own generator.
    ///
    /// Randomness arrives the way time does: from the edge, as a fact, so that
    /// nothing inside reads a source of its own and a replay of the log lands
    /// where it did the first time. The machine draws sign-in codes and
    /// session tokens from it and can mint neither until one arrives.
    ///
    /// Sent at startup and on a timer. Reseeding is what bounds the damage of
    /// a leaked log: a seed only tells you the codes drawn before the next one.
    Seeded { seed: [u8; 32] },

    /// What time it is now.
    ///
    /// The clock is an event rather than an argument to `handle_events`, and
    /// that is the whole point: nothing inside the machine reads one, so a
    /// replay of the log lands where it did the first time. It arrives about
    /// once a second, which is far finer than anything here measures — the
    /// shortest window in the machine is thirty seconds.
    ///
    /// It is also what lets the machine notice that *nothing* happened. A call
    /// that never answers produces no event, so without a tick there is no
    /// turn in which to give up on it, and the route waiting would wait
    /// forever.
    Tick { now: Timestamp },
}
