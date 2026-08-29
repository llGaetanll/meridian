//! Something to send back to a client.

#[derive(Debug)]
pub enum ActionClient {
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

    /// The code was not good. Deliberately carries no reason: wrong, expired
    /// and exhausted are logged and are not told apart to the caller, because
    /// only somebody working through guesses benefits from knowing which.
    Rejected,
}
