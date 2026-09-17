//! What a verified webhook said, and what the store knows about it.

use crate::ItemFacts;

/// One webhook, after the runtime has believed it and read what it names.
///
/// [`PlaidWebhookKind`](src_plaid::PlaidWebhookKind) plus the facts the
/// machine needs and does not keep: a webhook names a bank rather than a
/// person. One that failed verification never becomes one of these.
#[derive(Debug)]
pub enum WebhookFacts {
    /// One bank has transactions we have not read.
    ///
    /// `item` is what the store holds for it, or `None` for a bank we have no
    /// row for — Plaid keeps sending about an Item after the row went.
    SyncUpdatesAvailable {
        item_id: String,
        item: Option<ItemFacts>,
    },

    /// One bank has stopped answering until its owner signs in again, or has
    /// started answering because they did. `required` is which.
    Login { item_id: String, required: bool },

    /// Something we do not act on.
    ///
    /// Taken and ignored rather than refused: Plaid retries anything not
    /// answered 2xx, so an error here is a webhook that arrives again.
    Ignored {
        webhook_type: String,
        webhook_code: String,
    },
}
