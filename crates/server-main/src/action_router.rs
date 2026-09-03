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

    /// The request was taken, and nothing more is owed about it.
    ///
    /// Two routes answer with it and both mean it strictly. `/auth/request`
    /// does not mean "an email was sent": the address may be one nobody has,
    /// and that is deliberately indistinguishable from one that works — see
    /// the route's own note. `/plaid/webhook` does not mean "and it was acted
    /// on": a type nothing handles is taken too, because Plaid retries
    /// anything not answered 2xx.
    Accepted,

    /// The machine cannot answer this yet. So far only one thing causes it:
    /// being asked for a credential before any entropy has arrived, which
    /// there is no safe fallback for.
    Unavailable,

    /// They are signed in, and this is the token that says so.
    ///
    /// The token is on the action because the runtime has no other way to
    /// learn it — the machine generated it, from the seeded RNG, and this is
    /// the only place it crosses.
    SignedIn { token: String },

    /// Send them to Plaid's page to sign into their bank.
    LinkOpened { url: String },

    /// How many banks the Link flow attached.
    ///
    /// A count and not a yes: one `/link/finish` collects *every* session the
    /// caller has open, so it can attach two, and a bool would be a lossy
    /// summary of what the route actually did. Zero is the ordinary outcome
    /// of somebody closing the sheet — an answer, not a failure.
    ///
    /// It does not name them. At this point nobody knows what the bank is
    /// called: the name arrives with the balances call inside the first sync,
    /// seconds later, and reaches the screen through
    /// [`SyncStatus`](ActionRouter::SyncStatus). Waiting for it here would
    /// hold the sheet open for a string.
    Linked { count: u32 },

    /// Plaid has not said what the Link session came to.
    ///
    /// The third answer, and the honest one: the redirect that closed the
    /// sheet means nothing on its own, so a server that has not heard from
    /// Plaid yet knows neither that a bank was attached nor that none was.
    /// Saying so beats guessing between the two, and the screen it sends
    /// somebody to is the one that polls and will see a late bank arrive.
    LinkPending,

    /// How far the sync this user is watching has got.
    ///
    /// Assembled here rather than read off the runtime, because unlike a
    /// snapshot there is nothing in the store to read: progress exists only
    /// in the machine and only while a fetch is running.
    SyncStatus { status: server_wire::WireSyncStatus },

    /// The code was not good. Deliberately carries no reason: wrong, expired
    /// and exhausted are logged and are not told apart to the caller, because
    /// only somebody working through guesses benefits from knowing which.
    Rejected,
}
