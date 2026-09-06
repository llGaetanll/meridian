//! What the store knows about one bank, as a request finds it.

use api_common::Timestamp;
use uuid::Uuid;

/// One Item, as the runtime read it out of the store a moment ago.
///
/// Two users share a process and do not share a cursor, so these travel *with
/// the request* rather than being held between them. The one exception is
/// noted on the field that is it.
#[derive(Debug, Clone)]
pub struct ItemFacts {
    pub item_id: String,

    /// Whose bank it is. The one fact here that outlives the request: a sync
    /// runs on after the answer has gone out, and what it reports has to reach
    /// the right person.
    pub user_id: Uuid,

    /// Where the last sync reached. `None` starts from the beginning.
    pub cursor: Option<String>,

    /// When that sync was. `None` means never.
    pub synced_at: Option<Timestamp>,

    /// Whether Plaid has said this Item needs the user to sign in again. No
    /// amount of asking fixes it.
    pub login_required: bool,
}
