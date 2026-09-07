//! What a request was doing, while a read it needs is out.

use uuid::Uuid;

use crate::Authenticated;
use crate::ClientIp;

/// A request parked on a read, and what it will do with the answer.
///
/// Only what the machine cannot get back: who is waiting is already in
/// [`Calls`](crate::Calls), keyed by the same call.
#[derive(Debug)]
pub enum Pending {
    /// A sign-in, waiting on the code the store holds against its address.
    Submitted { email: String, code: String },

    /// A request waiting on the session that says who is asking. The address
    /// rides along because the rate limit needs both halves of the caller.
    Authenticating { ip: ClientIp, event: Authenticated },

    /// A webhook waiting on the key it says it was signed with. Held whole,
    /// because nothing about it can be believed until the key arrives.
    WebhookKey { jwt: String, body: Vec<u8> },

    /// A believed webhook waiting on the bank it names.
    WebhookItem { item_id: String },

    /// A request whose caller is known, waiting on the rows it decides from.
    Loading { user_id: Uuid, event: Authenticated },
}
