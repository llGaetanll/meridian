//! The answer a route is waiting on.
//!
//! Exactly one of these per [`EventRouter`](crate::EventRouter). The handler
//! parks until its own comes back, so a router event that produced none would
//! be a request that never answers.

#[derive(Debug)]
pub enum ActionRouter {
    /// Answer with what this user's store holds.
    ///
    /// The snapshot is assembled by the runtime rather than carried here: it
    /// is a read of somebody's ledger, and copying it into an action would
    /// mean building it even when nobody is waiting for one.
    Serve,

    /// They are signed in, and this is the token that says so.
    ///
    /// The token is on the action because the runtime has no other way to
    /// learn it — the machine generated it, from the seeded RNG, and this is
    /// the only place it crosses.
    SignedIn { token: String },

    /// Send them to Plaid's page to sign into their bank.
    LinkOpened { url: String },

    /// The Link flow ended with a bank attached.
    Linked,

    /// It ended without one, which is ordinary.
    NotLinked,

    /// The code was not good. Deliberately carries no reason: wrong, expired
    /// and exhausted are logged and are not told apart to the caller, because
    /// only somebody working through guesses benefits from knowing which.
    Rejected,
}
