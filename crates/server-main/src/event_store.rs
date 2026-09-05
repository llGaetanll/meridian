//! The store answered, or failed to.
//!
//! Both report back. A read carries what it found, because the machine asked
//! for it and cannot decide without it. A write carries only that it landed,
//! because that is the whole of what anything decides about one: a route is
//! released once what it started has been written.

use crate::CallId;
use crate::StoredCode;

#[derive(Debug)]
pub enum EventStore {
    /// Who a session token signs in. `None` is a token that names nobody —
    /// wrong, expired, or signed out — which is one answer and not three: the
    /// difference is only useful to somebody working through tokens.
    SessionResolved {
        call_id: CallId,
        user_id: Option<uuid::Uuid>,
    },

    /// What stands against the address a sign-in named. `None` is an address
    /// with no code, which is a rejection like any other.
    CodeLoaded {
        call_id: CallId,
        stored: Option<StoredCode>,
    },

    /// A read the store would not answer. Its own event rather than an empty
    /// answer: "no code" and "we could not look" are different sentences, and
    /// only one of them is the caller's fault.
    ReadError { call_id: CallId, message: String },

    /// The write landed. The rows a route is about to read are the new ones.
    WriteSuccess { call_id: CallId },

    /// The write did not land, and why. What was fetched is gone, and the
    /// cursor not having moved is what makes that recoverable rather than a
    /// hole: the next sync re-reads the same range.
    WriteError { call_id: CallId, message: String },
}
