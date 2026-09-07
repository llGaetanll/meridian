//! How much the machine is holding, counted.
//!
//! Everything in [`StateServer`](crate::StateServer) that can grow is a map
//! keyed by something that arrives from outside — a call, a request, a bank, a
//! link session, a `kid` off a stranger's token, a caller's address. Each one
//! is filled on the way in and emptied on the way out, and none of the
//! emptying reaches an action: a flow that forgets to put an entry back is a
//! flow whose `eval!` block passes and whose process grows for its life.
//!
//! A census is the one thing that says so. Take one before a flow, take one
//! after it has finished and the machine has been left to idle, and compare.

/// How many entries the machine holds, one number per thing that can grow.
///
/// Counts rather than contents. What a leak asks is how many, and a census
/// carrying the entries themselves would be a way to read state that the
/// events are meant to be the only account of.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CensusServer {
    /// Calls in flight, each one a route parked on an answer. Emptied as the
    /// answers land, and by the tick that gives up on the ones that do not.
    pub waiting: usize,

    /// Routes in flight, each at the step it has reached. Emptied by the
    /// answer going out, whichever turn that is.
    pub requests: usize,

    /// Banks with a fetch running, or with one whose result nothing has read
    /// yet.
    pub syncing: usize,

    /// Streams open, each one a client being told how its sync is going.
    /// Emptied by the socket closing, and capped per user by
    /// [`WATCHERS_AT_MOST`](crate::WATCHERS_AT_MOST).
    pub watchers: usize,

    /// Link sessions with one half of the race in: reported and not asked
    /// about, or asked about and not reported. Emptied by the other half, or
    /// by the tick that stops waiting for it.
    pub sessions: usize,

    /// Banks waiting on the write that attaches them to somebody. Emptied by
    /// that write answering, however many attempts it takes, and by the tick
    /// that gives up on one that never does.
    pub attaching: usize,

    /// Webhook signing keys remembered, misses included. A key is kept for
    /// the life of the process — Plaid gives a rotated one a new id — so this
    /// one only has to stay under
    /// [`WEBHOOK_KEYS_AT_MOST`](crate::WEBHOOK_KEYS_AT_MOST).
    pub keys: usize,

    /// Rate-limit windows kept against callers of this server. Swept on the
    /// tick, because the keys are addresses, sessions and inboxes that
    /// whoever calls us chooses.
    pub request_windows: usize,

    /// Rate-limit windows kept against Plaid, one per endpoint per bank.
    pub plaid_windows: usize,
}
