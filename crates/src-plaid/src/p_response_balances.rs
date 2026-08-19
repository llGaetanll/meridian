//! What `/accounts/balance/get` answers with.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct BalancesResponse {
    pub accounts: Vec<PlaidAccount>,

    /// Which Item these accounts belong to, and whether it is healthy.
    pub item: PlaidItem,
}

/// One account, as Plaid reports it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaidAccount {
    pub account_id: String,

    /// The bank's name for it, e.g. "Plaid Checking".
    pub name: String,

    /// A longer official name, where the bank has one.
    #[serde(default)]
    pub official_name: Option<String>,

    /// `depository`, `credit`, `loan`, `investment`, `brokerage`, `other`.
    ///
    /// **This is what says whether a balance is held or owed** — the sign
    /// never does. A card and a mortgage both report a positive `current`.
    #[serde(rename = "type")]
    pub kind: String,

    /// `checking`, `401k`, `home equity`, and so on. Absent on some accounts.
    #[serde(default)]
    pub subtype: Option<String>,

    /// The last four digits.
    #[serde(default)]
    pub mask: Option<String>,

    /// Interest paid, where the account pays any. Plaid reports `0` rather
    /// than null for accounts that pay nothing.
    #[serde(default)]
    pub apy: Option<f64>,

    pub balances: PlaidBalances,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaidBalances {
    /// The balance. Positive on a loan means that much is owed.
    #[serde(default)]
    pub current: Option<f64>,

    /// Spendable now — the balance minus holds, or the credit still free.
    #[serde(default)]
    pub available: Option<f64>,

    /// A credit line's ceiling.
    #[serde(default)]
    pub limit: Option<f64>,

    #[serde(default)]
    pub iso_currency_code: Option<String>,
}

/// The Item, and its error if it has one.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaidItem {
    pub item_id: String,

    #[serde(default)]
    pub institution_id: Option<String>,

    /// Present since 2024 on most responses; the server falls back to the id
    /// when it is missing.
    #[serde(default)]
    pub institution_name: Option<String>,

    /// A standing problem with the connection. `ITEM_LOGIN_REQUIRED` here
    /// means every future sync fails until the user reconnects.
    #[serde(default)]
    pub error: Option<PlaidItemError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaidItemError {
    #[serde(default)]
    pub error_code: Option<String>,

    #[serde(default)]
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed to the fields we read, but shaped like the real answer —
    /// including `limit: null` on a depository account and a `subtype` we do
    /// not have a special case for.
    const BODY: &str = r#"{
        "accounts": [
            {
                "account_id": "acct-1",
                "name": "Plaid Checking",
                "official_name": "Plaid Gold Standard 0% Interest Checking",
                "type": "depository",
                "subtype": "checking",
                "mask": "0000",
                "apy": 0,
                "balances": {
                    "available": 100,
                    "current": 110,
                    "limit": null,
                    "iso_currency_code": "USD"
                }
            },
            {
                "account_id": "acct-2",
                "name": "Plaid Mortgage",
                "type": "loan",
                "subtype": "mortgage",
                "mask": "8888",
                "balances": { "current": 56302.06, "available": null, "limit": null }
            }
        ],
        "item": {
            "item_id": "item-1",
            "institution_id": "ins_109508",
            "institution_name": "First Platypus Bank",
            "error": null
        }
    }"#;

    #[test]
    fn parses_balances() {
        let res: BalancesResponse = serde_json::from_str(BODY).unwrap();

        assert_eq!(res.accounts.len(), 2);
        assert_eq!(
            res.item.institution_name.as_deref(),
            Some("First Platypus Bank")
        );

        let checking = &res.accounts[0];
        assert_eq!(checking.kind, "depository");
        assert_eq!(checking.balances.current, Some(110.0));
        assert_eq!(checking.balances.limit, None);

        // The loan reports a *positive* balance. Only `type` says it is owed.
        let mortgage = &res.accounts[1];
        assert_eq!(mortgage.kind, "loan");
        assert_eq!(mortgage.balances.current, Some(56_302.06));
    }

    #[test]
    fn an_item_error_parses() {
        let body = r#"{
            "accounts": [],
            "item": {
                "item_id": "item-1",
                "error": { "error_code": "ITEM_LOGIN_REQUIRED", "error_message": "…" }
            }
        }"#;

        let res: BalancesResponse = serde_json::from_str(body).unwrap();

        assert_eq!(
            res.item.error.unwrap().error_code.as_deref(),
            Some("ITEM_LOGIN_REQUIRED")
        );
    }
}
