//! Something for a source to do.

#[derive(Debug)]
pub enum ActionSource {
    /// Go and ask Plaid about one Item.
    ///
    /// Names the Item rather than leaving the runtime to pick one. The access
    /// token is the runtime's — it never enters an action, because an action
    /// that carried one would put a bank credential in the event log — so this
    /// is how the runtime knows *which* token to reach for.
    ///
    /// The cursor is where to resume from, if there is one. Losing it means
    /// re-fetching an entire history.
    FetchPlaid {
        item_id: String,
        cursor: Option<String>,
    },
}
