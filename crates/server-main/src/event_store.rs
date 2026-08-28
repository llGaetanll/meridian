//! The store answered, or failed to.

use api_common::Timestamp;
use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;

#[derive(Debug)]
pub enum EventStore {
    /// Everything that was held from last time.
    ///
    /// Empty is an ordinary answer — a first run, or a server with no
    /// `DATABASE_URL` — and is not the same as a failure. Both leave the ledger
    /// empty; only one of them means something is wrong.
    Loaded {
        entries: Vec<Entry>,
        connections: Vec<Connection>,
        streams: Vec<Stream>,

        /// Where the last sync reached. `None` means the next one starts from
        /// the beginning.
        cursor: Option<String>,

        /// When that sync was, so a restart inside the freshness window does
        /// not go straight back to Plaid for data it already has.
        synced_at: Option<Timestamp>,
    },

    /// Nothing could be read. The server carries on with an empty ledger and
    /// fetches, which is the same thing a first run does.
    LoadFailed { message: String },

    /// Nothing could be written. What was fetched is still in memory and still
    /// being served; it is the *next restart* that would notice, and the cursor
    /// not having moved is what makes that recoverable rather than a hole.
    SaveFailed { message: String },
}
