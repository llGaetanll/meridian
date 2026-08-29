//! What the store knows about one bank, as a request finds it.

use api_common::Timestamp;

/// One Item, as the runtime read it out of the store a moment ago.
///
/// These used to be fields on [`StateServer`](crate::StateServer) — one
/// cursor, one `fetched_at`, one login flag, for the whole server. That was
/// only ever right because there was one user and one bank. Two users share a
/// process and do not share a cursor, so the facts travel *with the request*
/// and the machine keeps none of them.
///
/// What the machine still owns is the decision: given these, and what it has
/// already spent against Plaid's rate limit, is another call warranted.
#[derive(Debug, Clone)]
pub struct ItemFacts {
    pub item_id: String,

    /// Where the last sync reached. `None` starts from the beginning.
    pub cursor: Option<String>,

    /// When that sync was. `None` means never.
    pub synced_at: Option<Timestamp>,

    /// Whether Plaid has said this Item needs the user to sign in again. No
    /// amount of asking fixes it, so it outranks even a forced refresh.
    pub login_required: bool,
}
