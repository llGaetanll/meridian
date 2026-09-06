//! The store answered, or failed to.
//!
//! A read carries what it found; a write carries only that it landed.

use crate::CallId;
use crate::StoredCode;

#[derive(Debug)]
pub enum EventStore {
    /// Who a session token signs in. `None` is a token that names nobody —
    /// wrong, expired, or signed out, deliberately not told apart.
    SessionResolved {
        call_id: CallId,
        user_id: Option<uuid::Uuid>,
    },

    /// One bank a webhook named, as the store holds it. `None` is a bank we
    /// have no row for — Plaid keeps sending after one is removed.
    ItemLoaded {
        call_id: CallId,
        item: Option<crate::ItemFacts>,
    },

    /// This user's banks, as the store holds them. Empty is a first run, not a
    /// failure.
    ItemsLoaded {
        call_id: CallId,
        items: Vec<crate::ItemFacts>,
    },

    /// The Link sessions this user has open. Empty is a sheet closed on a
    /// session we have no record of, or one already collected.
    PendingLinksLoaded {
        call_id: CallId,
        link_tokens: Vec<String>,
    },

    /// The ledger a snapshot is about to be answered with, as a value: nothing
    /// outside the machine reads the store.
    SnapshotLoaded {
        call_id: CallId,
        snapshot: server_wire::WireSnapshot,
    },

    /// What stands against the address a sign-in named. `None` is an address
    /// with no code, which is a rejection like any other.
    CodeLoaded {
        call_id: CallId,
        stored: Option<StoredCode>,
    },

    /// A read the store would not answer. Its own event rather than an empty
    /// answer: "no code" and "we could not look" are different sentences.
    ReadError { call_id: CallId, message: String },

    /// The write landed.
    WriteSuccess { call_id: CallId },

    /// The write did not land, and why. Recoverable because the cursor did not
    /// move: the next sync re-reads the same range.
    WriteError { call_id: CallId, message: String },
}
