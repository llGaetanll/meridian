//! A client asked for something.

use crate::ItemFacts;
use crate::StoredCode;
use uuid::Uuid;

#[derive(Debug)]
pub enum EventClient {
    /// Someone wants their snapshot.
    ///
    /// Carries the banks the store holds for them, read by the runtime just
    /// before this event was made. The machine keeps no cursors of its own —
    /// two users share a process and do not share a cursor — so the facts a
    /// decision needs arrive with the request that needs one.
    ///
    /// `force` is pull-to-refresh, which skips the freshness window.
    SnapshotRequested { force: bool, items: Vec<ItemFacts> },

    /// Somebody wants a sign-in code for this address.
    ///
    /// Carries no code. The machine makes it, from the seeded RNG, which is
    /// what keeps "what code did we send?" answerable from the event log.
    CodeRequested { email: String },

    /// Somebody typed a code.
    ///
    /// `stored` is what the store holds against that address, read by the
    /// runtime just before this event was made — the same shape as
    /// [`ItemFacts`](crate::ItemFacts), and for the same reason. `None` is an
    /// address with no code standing, which is a rejection like any other.
    CodeSubmitted {
        email: String,
        code: String,
        stored: Option<StoredCode>,
    },

    /// Somebody wants to connect a bank. The id is theirs, so the session
    /// can be written down against them before its URL is handed over.
    LinkRequested { user_id: Uuid },

    /// The bank sign-in page closed, so whatever happened there has happened.
    ///
    /// Carries the sessions the store holds open for this user, read by the
    /// runtime just before — the same shape as
    /// [`ItemFacts`](crate::ItemFacts). The redirect that closed the sheet
    /// carries no token, so the result has to be collected, and this is what
    /// says it is worth collecting now.
    LinkClosed { pending: Vec<String> },
}
