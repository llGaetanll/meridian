use serde_json::Value;

use crate::PlaidRequest;

/// `/transactions/recurring/get` — the charges that come back.
///
/// Plaid does the detection, which is a real saving: it returns a frequency,
/// an average amount, a predicted next date and a status per stream, so the
/// Recurring screen is a render rather than a classifier of ours.
///
/// It needs the Item to have been synced first — the streams are derived from
/// the transactions, so a fresh Item answers with nothing until its history
/// has loaded.
pub struct RecurringRequest {
    pub access_token: String,

    /// Restrict to certain accounts. Empty means all of them, which is what
    /// the app wants.
    pub account_ids: Vec<String>,
}

impl RecurringRequest {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            account_ids: Vec::new(),
        }
    }
}

impl PlaidRequest for RecurringRequest {
    fn path(&self) -> String {
        String::from("transactions/recurring/get")
    }

    fn body(&self) -> Value {
        let mut body = serde_json::json!({ "access_token": self.access_token });

        // Omitted rather than empty: Plaid reads an empty array as "no
        // accounts" on some endpoints, and absent as "all" on all of them.
        if !self.account_ids.is_empty() {
            body["account_ids"] = serde_json::json!(self.account_ids);
        }

        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_accounts_means_the_field_is_absent() {
        let body = RecurringRequest::new("access-sandbox-1").body();

        assert_eq!(body["access_token"], "access-sandbox-1");
        assert!(body.get("account_ids").is_none());
    }
}
