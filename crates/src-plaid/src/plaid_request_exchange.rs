use serde_json::Value;

use crate::PlaidRequest;

/// `/item/public_token/exchange` — the one-way door.
///
/// A `public_token` is worth nothing on its own and expires in thirty minutes.
/// What comes back does not expire at all, and is the credential every later
/// call to this Item is made with. It is stored, and `docs/PLAID.md` is the
/// reason it is stored on the server: the phone never holds a token of either
/// kind.
pub struct ExchangeRequest {
    pub public_token: String,
}

impl ExchangeRequest {
    pub fn new(public_token: impl Into<String>) -> Self {
        Self {
            public_token: public_token.into(),
        }
    }
}

impl PlaidRequest for ExchangeRequest {
    fn path(&self) -> String {
        String::from("item/public_token/exchange")
    }

    fn body(&self) -> Value {
        serde_json::json!({ "public_token": self.public_token })
    }
}
