//! The store answered, or failed to.
//!
//! A successful *read* is not an event because nothing decides anything about
//! it: the runtime reads a user's rows on the request path and serves them,
//! and the only thing the machine wants from the store is the handful of facts
//! on [`EventRouter::SnapshotRequested`](crate::EventRouter).
//!
//! A successful *write* is, because something does decide about it. A route is
//! told to read the store only once what it is waiting on has landed, and
//! "landed" is a fact the runtime reports rather than one the machine can
//! assume.

use crate::CallId;

#[derive(Debug)]
pub enum EventStore {
    /// The write landed. The rows a route is about to read are the new ones.
    WriteSuccess { call_id: CallId },

    /// The write did not land, and why. What was fetched is gone, and the
    /// cursor not having moved is what makes that recoverable rather than a
    /// hole: the next sync re-reads the same range.
    WriteError { call_id: CallId, message: String },
}
