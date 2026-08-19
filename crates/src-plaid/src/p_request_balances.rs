use serde_json::Value;

use crate::PRequest;

/// `/accounts/balance/get` — what every account currently holds.
///
/// Distinct from `/accounts/get`, which answers from Plaid's cache. This one
/// asks the bank, so it is the endpoint that can say a balance moved since the
/// last sync. It is also the slower and rate-limited one, which is why the
/// server calls it on a refresh rather than on every page load.
pub struct BalancesRequest {
    pub access_token: String,
}

impl BalancesRequest {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
        }
    }
}

impl PRequest for BalancesRequest {
    fn path(&self) -> String {
        String::from("accounts/balance/get")
    }

    fn body(&self) -> Value {
        serde_json::json!({ "access_token": self.access_token })
    }
}
