//! What the store knows about one bank, as a request finds it.

use api_common::Timestamp;

/// One Item, as the runtime read it out of the store a moment ago.
///
/// Two users share a process and do not share a cursor, so these travel *with
/// the request* and the machine keeps none of them.
#[derive(Debug, Clone)]
pub struct ItemFacts {
    pub item_id: String,

    /// Where the last sync reached. `None` starts from the beginning.
    pub cursor: Option<String>,

    /// When that sync was. `None` means never.
    pub synced_at: Option<Timestamp>,

    /// Whether Plaid has said this Item needs the user to sign in again. No
    /// amount of asking fixes it.
    pub login_required: bool,
}
