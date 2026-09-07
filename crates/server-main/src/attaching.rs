//! A bank waiting on the write that attaches it to somebody.

/// One `SaveItem` in flight, and what it would take to ask for it again.
///
/// Keyed by the call the write is out on, because that is what the store's
/// answer names. It holds what the action held rather than a reference to it:
/// the retry is a fresh call for the same two strings, and the access token
/// they stand for never leaves the runtime.
///
/// The reason it exists at all is that the exchange in front of it cannot be
/// undone. A `public_token` traded for an access token has spent one of the
/// account's Production Item slots, and `/item/remove` does not give one back,
/// so a write that failed is worth asking for again where an ordinary one is
/// not.
#[derive(Debug)]
pub struct Attaching {
    pub item_id: String,

    /// The Link session the bank arrived on, which is what says whose it is.
    pub link_token: String,

    /// Which attempt this call is, counting the first. Capped by
    /// [`ATTACH_ATTEMPTS`](crate::ATTACH_ATTEMPTS).
    pub attempt: u32,
}
