//! A client asked for something.

use crate::ItemFacts;

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
}
