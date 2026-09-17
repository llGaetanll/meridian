use serde_json::Value;

use crate::PlaidRequest;

/// `/accounts/get` — every account and its balance, from Plaid's cache.
///
/// Distinct from `/accounts/balance/get`, which asks the bank live and is
/// billed per successful call. The cache refreshes about daily on an Item
/// with Transactions, balances go stale in hours rather than seconds, and
/// this one is free — which is why it is the endpoint the routine sync path
/// uses. Nothing in the app calls the billed one.
pub struct AccountsRequest {
    pub access_token: String,
}

impl AccountsRequest {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
        }
    }
}

impl PlaidRequest for AccountsRequest {
    fn path(&self) -> String {
        String::from("accounts/get")
    }

    fn body(&self) -> Value {
        serde_json::json!({ "access_token": self.access_token })
    }
}
