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
    /// It is written. The rows a route is about to read are the new ones.
    Written { call_id: CallId },

    /// Nothing could be written. What was fetched is gone, and the cursor not
    /// having moved is what makes that recoverable rather than a hole: the
    /// next sync re-reads the same range.
    ///
    /// It ends the wait the same way [`Written`](Self::Written) does. A route
    /// held until a write that failed would be held forever, and serving the
    /// rows we already had is the right answer to a sync that did not land.
    SaveFailed { call_id: CallId, message: String },
}
