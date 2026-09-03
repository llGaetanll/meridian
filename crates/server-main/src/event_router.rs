//! A route was asked for something.
//!
//! One variant per route that dispatches, and the handler for that route is
//! what makes it. "Router" rather than "client": the phone is a client too,
//! and so is the web app, but only a handler is waiting on the answer.

use crate::ItemFacts;
use crate::StoredCode;
use crate::WebhookFacts;
use uuid::Uuid;

#[derive(Debug)]
pub enum EventRouter {
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

    /// Somebody is watching a sync and wants to know how far it has got.
    ///
    /// Carries their banks for the same reason `SnapshotRequested` does: the
    /// machine holds progress by item and has no idea whose item is whose.
    SyncStatusRequested { items: Vec<ItemFacts> },

    /// The bank sign-in page closed, so whatever happened there has happened.
    ///
    /// Carries the sessions the store holds open for this user, read by the
    /// runtime just before — the same shape as
    /// [`ItemFacts`](crate::ItemFacts). The redirect that closed the sheet
    /// carries no token, so the result has to be collected, and this is what
    /// says it is worth collecting now.
    ///
    /// With `PLAID_COLLECT=webhook` it stops being a trigger and becomes a
    /// question: collection happens unprompted, so the sessions are what the
    /// machine matches Plaid's `SESSION_FINISHED` against rather than what it
    /// goes and polls.
    LinkClosed { pending: Vec<String> },

    /// Plaid called us, and the call was believed.
    ///
    /// Every check is behind this event — the algorithm, the signature, the
    /// replay window and the body hash — the same way `user_id` on
    /// [`LinkRequested`](EventRouter::LinkRequested) is behind a session
    /// lookup. The machine decides what a webhook *means*, never whether it
    /// is really Plaid's.
    WebhookReceived { webhook: WebhookFacts },
}
