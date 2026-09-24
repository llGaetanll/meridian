//! What `/investments/holdings/get` answers with.

use api_common::Timestamp;
use serde::Deserialize;

use crate::PlaidAccount;
use crate::PlaidItem;
use crate::PlaidSecurity;
use crate::optional_date;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HoldingsResponse {
    /// The investment accounts the holdings sit in. Same shape as
    /// `/accounts/get`, so the balances ride along.
    pub accounts: Vec<PlaidAccount>,

    pub holdings: Vec<PlaidHolding>,

    /// Every security the holdings point at, keyed by `security_id`.
    pub securities: Vec<PlaidSecurity>,

    pub item: PlaidItem,
}

/// One position: this much of that security, in this account.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaidHolding {
    pub account_id: String,

    /// Which security — the instrument itself is in
    /// [`HoldingsResponse::securities`].
    pub security_id: String,

    /// Units held. Shares for an equity, face value for a bond, coins for a
    /// cryptocurrency.
    pub quantity: f64,

    /// The institution's own last price, as of `institution_price_as_of` —
    /// usually yesterday's close.
    pub institution_price: f64,

    #[serde(default, deserialize_with = "optional_date")]
    pub institution_price_as_of: Option<Timestamp>,

    /// `quantity` times `institution_price`, as the institution reports it.
    pub institution_value: f64,

    /// What was paid for the position, where the institution says.
    #[serde(default)]
    pub cost_basis: Option<f64>,

    #[serde(default)]
    pub iso_currency_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed to the fields we read, but shaped like the real answer — a
    /// share position and the brokerage's cash row, which is also a "holding".
    const BODY: &str = r#"{
        "accounts": [
            {
                "account_id": "acct-1",
                "name": "Plaid IRA",
                "type": "investment",
                "subtype": "ira",
                "mask": "6666",
                "balances": { "current": 320.76, "available": null, "limit": null, "iso_currency_code": "USD" }
            }
        ],
        "holdings": [
            {
                "account_id": "acct-1",
                "security_id": "sec-1",
                "quantity": 10,
                "institution_price": 11,
                "institution_price_as_of": "2026-09-23",
                "institution_value": 110,
                "cost_basis": 87.5,
                "iso_currency_code": "USD",
                "unofficial_currency_code": null
            },
            {
                "account_id": "acct-1",
                "security_id": "sec-cash",
                "quantity": 12.5,
                "institution_price": 1,
                "institution_price_as_of": null,
                "institution_value": 12.5,
                "cost_basis": null,
                "iso_currency_code": "USD"
            }
        ],
        "securities": [
            {
                "security_id": "sec-1",
                "isin": "US0378331005",
                "cusip": "037833100",
                "name": "Apple Inc.",
                "ticker_symbol": "AAPL",
                "is_cash_equivalent": false,
                "type": "equity",
                "close_price": 11,
                "close_price_as_of": "2026-09-23",
                "iso_currency_code": "USD"
            },
            {
                "security_id": "sec-cash",
                "name": "U S Dollar",
                "ticker_symbol": "USD",
                "is_cash_equivalent": true,
                "type": "cash",
                "close_price": 1,
                "iso_currency_code": "USD"
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
    fn parses_holdings_and_their_securities() {
        let res: HoldingsResponse = serde_json::from_str(BODY).unwrap();

        assert_eq!(res.holdings.len(), 2);
        assert_eq!(res.securities.len(), 2);
        assert_eq!(res.accounts[0].kind, "investment");

        let apple = &res.holdings[0];
        assert_eq!(apple.quantity, 10.0);
        assert_eq!(apple.institution_value, 110.0);
        assert!(apple.institution_price_as_of.is_some());

        let security = &res.securities[0];
        assert_eq!(security.label(), "AAPL");
        assert_eq!(security.kind.as_deref(), Some("equity"));
    }

    /// The brokerage's uninvested cash arrives as a holding like any other,
    /// priced at 1. Only the security says it is cash.
    #[test]
    fn cash_is_a_holding_too() {
        let res: HoldingsResponse = serde_json::from_str(BODY).unwrap();

        let cash = &res.holdings[1];
        assert_eq!(cash.institution_price, 1.0);
        assert_eq!(cash.cost_basis, None);

        let security = &res.securities[1];
        assert_eq!(security.is_cash_equivalent, Some(true));
        assert_eq!(security.kind.as_deref(), Some("cash"));
    }
}
