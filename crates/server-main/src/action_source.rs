//! Something for a source to do.

#[derive(Debug)]
pub enum ActionSource {
    /// Go and ask Plaid. Carries the cursor to resume from, if there is one —
    /// losing it means re-fetching an entire history.
    FetchPlaid { cursor: Option<String> },
}
