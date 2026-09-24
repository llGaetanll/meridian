//! The shape Plaid answers `/investments/transactions/get` with.
//!
//! Plaid's types as Plaid reports them, like `plaid_transaction`. The sign
//! convention is noted where it appears and resolved by whatever reads these
//! into a ledger.

use api_common::Timestamp;
use serde::Deserialize;

use crate::PlaidAccount;
use crate::PlaidItem;
use crate::PlaidSecurity;
use crate::date;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InvestmentsResponse {
    pub accounts: Vec<PlaidAccount>,

    /// One page, newest first.
    pub investment_transactions: Vec<PlaidInvestmentTransaction>,

    /// Every security this page's transactions point at.
    pub securities: Vec<PlaidSecurity>,

    /// How many the whole window holds. More than the page carries means
    /// another page, `offset` further in.
    pub total_investment_transactions: u32,

    pub item: PlaidItem,
}

/// One investment transaction, as Plaid reports it.
///
/// A trade is one security against cash, never two securities: buying ten
/// shares is `quantity: 10` of that security and a positive `amount` of the
/// account's currency. The other asset the user sees in a trade is always the
/// cash leg.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaidInvestmentTransaction {
    pub investment_transaction_id: String,
    pub account_id: String,

    /// Which security, for the rows about one. A fee or a cash movement can
    /// name none.
    #[serde(default)]
    pub security_id: Option<String>,

    /// The day it settled. **A day, not an instant** — Plaid reports no time
    /// of day for most institutions, so midnight UTC here means "sometime
    /// that day".
    #[serde(deserialize_with = "date")]
    pub date: Timestamp,

    /// The institution's description of it.
    #[serde(default)]
    pub name: Option<String>,

    /// Units moved. **Positive is units in** — a buy — **and negative is
    /// units out**, the opposite sign to `amount`, because a trade moves the
    /// security and the cash in opposite directions.
    pub quantity: f64,

    /// Cash moved. **Positive means money left the account**, the same
    /// convention as `PlaidTransaction`: a buy is positive, a sell negative.
    pub amount: f64,

    /// Per-unit price of the trade.
    pub price: f64,

    /// The institution's fee on this transaction, where it reports one.
    #[serde(default)]
    pub fees: Option<f64>,

    /// `buy`, `sell`, `cash`, `fee`, `transfer`, `cancel`.
    #[serde(rename = "type")]
    pub kind: String,

    /// The finer bucket: `dividend`, `contribution`, `management fee`, and so
    /// on.
    #[serde(default)]
    pub subtype: Option<String>,

    #[serde(default)]
    pub iso_currency_code: Option<String>,

    /// On a `cancel`, the id of the transaction it cancels — which must then
    /// be dropped, the same way a pending duplicate is.
    #[serde(default)]
    pub cancel_transaction_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed to the fields we read, but shaped like the real answer — a
    /// buy, and a dividend that names a security but moved no units.
    const BODY: &str = r#"{
        "accounts": [
            {
                "account_id": "acct-1",
                "name": "Plaid IRA",
                "type": "investment",
                "subtype": "ira",
                "balances": { "current": 320.76, "available": null, "limit": null }
            }
        ],
        "investment_transactions": [
            {
                "investment_transaction_id": "itx-1",
                "account_id": "acct-1",
                "security_id": "sec-1",
                "date": "2026-08-14",
                "name": "BUY Apple Inc.",
                "quantity": 10,
                "amount": 110.55,
                "price": 11,
                "fees": 0.55,
                "type": "buy",
                "subtype": "buy",
                "iso_currency_code": "USD",
                "cancel_transaction_id": null
            },
            {
                "investment_transaction_id": "itx-2",
                "account_id": "acct-1",
                "security_id": "sec-1",
                "date": "2026-08-01",
                "name": "Apple Inc. dividend",
                "quantity": 0,
                "amount": -4.25,
                "price": 0,
                "fees": null,
                "type": "cash",
                "subtype": "dividend",
                "iso_currency_code": "USD"
            }
        ],
        "securities": [
            {
                "security_id": "sec-1",
                "name": "Apple Inc.",
                "ticker_symbol": "AAPL",
                "is_cash_equivalent": false,
                "type": "equity",
                "close_price": 11,
                "close_price_as_of": "2026-09-23",
                "iso_currency_code": "USD"
            }
        ],
        "total_investment_transactions": 2,
        "item": {
            "item_id": "item-1",
            "institution_id": "ins_109508",
            "institution_name": "First Platypus Bank",
            "error": null
        }
    }"#;

    #[test]
    fn parses_a_page_of_trades() {
        let res: InvestmentsResponse = serde_json::from_str(BODY).unwrap();

        assert_eq!(res.total_investment_transactions, 2);
        assert_eq!(res.investment_transactions.len(), 2);
        assert_eq!(res.securities[0].label(), "AAPL");

        // The buy: units in, cash out, and both positive — the signs point
        // opposite ways on purpose.
        let buy = &res.investment_transactions[0];
        assert_eq!(buy.kind, "buy");
        assert_eq!(buy.quantity, 10.0);
        assert_eq!(buy.amount, 110.55);
        assert_eq!(buy.fees, Some(0.55));
    }

    /// A dividend is `cash` about a security: it names one, moves no units,
    /// and its negative amount means money arrived.
    #[test]
    fn a_dividend_moves_cash_and_no_units() {
        let res: InvestmentsResponse = serde_json::from_str(BODY).unwrap();

        let dividend = &res.investment_transactions[1];
        assert_eq!(dividend.kind, "cash");
        assert_eq!(dividend.subtype.as_deref(), Some("dividend"));
        assert_eq!(dividend.quantity, 0.0);
        assert!(dividend.amount < 0.0);
        assert_eq!(dividend.fees, None);
    }
}
