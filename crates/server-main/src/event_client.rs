//! A client asked for something.

use crate::ItemFacts;
use crate::StoredCode;

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
}
