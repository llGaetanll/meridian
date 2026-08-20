//! A client asked for something.

#[derive(Debug)]
pub enum EventClient {
    /// Someone wants the current snapshot. `force` is pull-to-refresh, which
    /// skips the freshness window.
    SnapshotRequested { force: bool },
}
