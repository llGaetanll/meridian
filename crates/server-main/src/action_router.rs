//! The answer a route is waiting on.
//!
//! One per [`EventRouter`](crate::EventRouter): the handler parks until its
//! own comes back, so an event that produced none never answers.

use crate::ConnId;

#[derive(Debug)]
pub enum ActionRouter {
    /// Answer with this user's ledger, carried whole because nothing outside
    /// the machine reads the store.
    Serve { snapshot: server_wire::WireSnapshot },

    /// The request was taken, and nothing more is owed about it. Strictly that
    /// much: not "an email was sent", and not "the webhook was acted on".
    Accepted,

    /// Nobody is signed in by the token this request carried. Which of wrong,
    /// expired or signed out is deliberately not told apart.
    Unauthorized,

    /// The machine cannot answer this yet — being asked for a credential
    /// before any entropy has arrived, which has no safe fallback.
    Unavailable,

    /// They are signed in, and this is the token that says so.
    ///
    /// The token is on the action because the machine generated it, from the
    /// seeded RNG, and this is the only place it crosses.
    SignedIn { token: String },

    /// Send them to Plaid's page to sign into their bank.
    LinkOpened { url: String },

    /// How many banks the Link flow attached, and never zero.
    ///
    /// A count and not a yes: one `/link/finish` collects *every* session the
    /// caller has open, so it can attach two. None attached is
    /// [`LinkAbandoned`](ActionRouter::LinkAbandoned) rather than a zero here.
    Linked { count: u32 },

    /// The Link session came to no bank — the ordinary ending, somebody closed
    /// the sheet, and not a failure.
    LinkAbandoned,

    /// Plaid has not said what the Link session came to. Neither that a bank
    /// was attached nor that none was, which beats guessing between the two.
    LinkPending,

    /// The stream is theirs, and this is where things stand on it.
    ///
    /// The status rides on the acceptance rather than following it, so a
    /// connection cannot be opened without being told where the sync has got
    /// to. A client that connects late, or reconnects, is then in the same
    /// position as one that was there from the start.
    Watching {
        conn_id: ConnId,
        status: server_wire::WireSyncStatus,
    },

    /// Too many requests, and this one is not being served.
    ///
    /// Carries nothing on purpose: told when the next call would be allowed, a
    /// caller behaving badly sits just inside the limit.
    TooMany,

    /// The code was not good. Deliberately carries no reason: wrong, expired
    /// and exhausted are logged and are not told apart to the caller.
    Rejected,
}
