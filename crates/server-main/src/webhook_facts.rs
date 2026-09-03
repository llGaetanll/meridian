//! What a verified webhook said, and what the store knows about it.

use crate::ItemFacts;

/// One webhook, after the runtime has believed it and read what it names.
///
/// Not [`PlaidWebhookKind`](src_plaid::PlaidWebhookKind), which is Plaid's two
/// strings turned into meaning. This is that plus the facts the machine needs
/// and does not keep — the same shape as [`ItemFacts`](crate::ItemFacts) and
/// for the same reason: a webhook names a bank rather than a person, so who
/// owns it and how far its last sync got are reads that happen before the
/// event is made.
///
/// A webhook that failed verification never becomes one of these. Believing it
/// is the runtime's job, exactly as resolving a bearer token to a user is: the
/// machine decides what to do about a request, never whether the request is
/// really from who it says.
#[derive(Debug)]
pub enum WebhookFacts {
    /// A Link session ended, and this is what it produced.
    ///
    /// The push half of what `PollLinks` pulls. Empty tokens is the ordinary
    /// outcome of somebody closing the sheet.
    SessionFinished {
        link_token: String,
        public_tokens: Vec<String>,
    },

    /// One bank has transactions we have not read.
    ///
    /// `item` is what the store holds for it, or `None` for a bank we have no
    /// row for. That happens: Plaid keeps sending about an Item after the row
    /// went, and there is nothing to sync against. It is a fact rather than an
    /// error, so it arrives as one and the machine decides.
    SyncUpdatesAvailable {
        item_id: String,
        item: Option<ItemFacts>,
    },

    /// One bank has stopped answering until its owner signs in again, or has
    /// started answering because they did. `required` is which.
    ///
    /// Written down either way, because it outlives the process: a server that
    /// restarts into a dead login it has forgotten asks Plaid the same question
    /// and is refused the same way.
    Login { item_id: String, required: bool },

    /// Something we do not act on.
    ///
    /// Taken and ignored rather than refused. Plaid sends types nobody planned
    /// for and retries anything not answered 2xx, so an error here is a webhook
    /// that arrives again in a minute, and again after that.
    Ignored {
        webhook_type: String,
        webhook_code: String,
    },
}
