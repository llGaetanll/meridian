use serde_json::Value;

use crate::PlaidRequest;

/// `/investments/holdings/get` — every position on the Item, from Plaid's
/// cache.
///
/// Covered by the Item's investments subscription rather than billed per
/// call. The prices in the answer are the institution's own and a day old at
/// best; the quantities are what this call is for, and a live portfolio value
/// is those quantities times our own market prices.
pub struct HoldingsRequest {
    pub access_token: String,
}

impl HoldingsRequest {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
        }
    }
}

impl PlaidRequest for HoldingsRequest {
    fn path(&self) -> String {
        String::from("investments/holdings/get")
    }

    fn body(&self) -> Value {
        serde_json::json!({ "access_token": self.access_token })
    }
}
