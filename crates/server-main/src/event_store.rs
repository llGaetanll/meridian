//! The store answered, or failed to.
//!
//! Only the failures are here. A successful read is not an event because
//! nothing decides anything about it: the runtime reads a user's rows on the
//! request path and serves them, and the only thing the machine wants from the
//! store is the handful of facts on
//! [`EventClient::SnapshotRequested`](crate::EventClient).

#[derive(Debug)]
pub enum EventStore {
    /// Nothing could be read. The user is served whatever came back, which on
    /// a failure is nothing — an empty ledger rather than an error page.
    LoadFailed { message: String },

    /// Nothing could be written. What was fetched is gone, and the cursor not
    /// having moved is what makes that recoverable rather than a hole: the
    /// next sync re-reads the same range.
    SaveFailed { message: String },
}
