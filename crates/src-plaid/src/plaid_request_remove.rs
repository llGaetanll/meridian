use serde_json::Value;

use crate::PlaidRequest;

/// `/item/remove` — stop the bank connection, and stop being billed for it.
///
/// **The only way a Plaid subscription stops.** An Item is billed monthly for
/// as long as it exists, not for as long as it is used, so a cancelled account
/// whose Items were left behind is a bill that arrives every month for nothing.
///
/// It does not give back the Production Item *slot*: the Trial plan allows ten
/// and this is not one of the ten returning. So removing is right for the bill
/// and irreversible for the cap — see `docs/BUSINESS.md`.
///
/// The access token stops working the moment this succeeds, which is also what
/// makes it the right last step of deleting an account: what is left in the
/// database afterwards cannot reach anybody's bank.
pub struct RemoveRequest {
    pub access_token: String,
}

impl RemoveRequest {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
        }
    }
}

impl PlaidRequest for RemoveRequest {
    fn path(&self) -> String {
        String::from("item/remove")
    }

    fn body(&self) -> Value {
        serde_json::json!({ "access_token": self.access_token })
    }
}
