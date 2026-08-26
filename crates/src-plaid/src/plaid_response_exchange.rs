//! What `/item/public_token/exchange` answers with.

use serde::Deserialize;

/// The permanent half of a bank connection.
///
/// **This is the response that spends a Production Item slot**, and the account
/// has ten of them — `/item/remove` does not give one back. Everything else in
/// this crate can be called again; this one cannot be undone.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExchangeResponse {
    /// Never expires, and grants read access to this Item until it is
    /// invalidated. Stored encrypted, never logged, never sent to a client.
    pub access_token: String,

    /// How Plaid names this Item in webhooks, so it is the key everything
    /// about the connection hangs off on our side too.
    pub item_id: String,
}
