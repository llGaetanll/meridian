//! What `/transactions/recurring/get` answers with.

use api_common::Timestamp;
use serde::Deserialize;

use crate::Category;
use crate::optional_date;

#[derive(Debug, Clone, Deserialize)]
pub struct RecurringResponse {
    /// Money going out — subscriptions, rent, card payments.
    #[serde(default)]
    pub outflow_streams: Vec<PlaidStream>,

    /// Money coming in — salary, interest.
    #[serde(default)]
    pub inflow_streams: Vec<PlaidStream>,
}

/// One detected stream.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaidStream {
    pub stream_id: String,

    pub account_id: String,

    /// The bank description shared by the transactions in this stream.
    pub description: String,

    /// The merchant, where Plaid matched one. Comes back as an empty string
    /// rather than null when it did not, which is why the projection checks
    /// for empty rather than for `None`.
    #[serde(default)]
    pub merchant_name: Option<String>,

    /// `WEEKLY`, `BIWEEKLY`, `SEMI_MONTHLY`, `MONTHLY`, `ANNUALLY`, `UNKNOWN`.
    pub frequency: String,

    /// The typical amount. Plaid's sign convention, so positive is money out
    /// on an outflow stream — inverted from the ledger's, same as everywhere
    /// else in this API.
    pub average_amount: StreamAmount,

    #[serde(default)]
    pub last_amount: Option<StreamAmount>,

    /// When Plaid expects the next one. Absent on a stream it thinks has
    /// stopped.
    #[serde(default, deserialize_with = "optional_date")]
    pub predicted_next_date: Option<Timestamp>,

    #[serde(default, deserialize_with = "optional_date")]
    pub last_date: Option<Timestamp>,

    /// `MATURE`, `EARLY_DETECTION`, `TOMBSTONED`, `UNKNOWN`.
    #[serde(default)]
    pub status: Option<String>,

    /// Whether Plaid still considers this stream live. A cancelled
    /// subscription keeps coming back from the endpoint, so counting an
    /// inactive stream would overstate the monthly commitment.
    #[serde(default)]
    pub is_active: Option<bool>,

    #[serde(default, rename = "personal_finance_category")]
    pub category: Option<Category>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StreamAmount {
    #[serde(default)]
    pub amount: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{
        "outflow_streams": [{
            "stream_id": "stream-1",
            "account_id": "acct-1",
            "description": "AUTOMATIC PAYMENT - THANK",
            "merchant_name": "",
            "frequency": "MONTHLY",
            "average_amount": { "amount": 2078.5 },
            "last_amount": { "amount": 2078.5 },
            "predicted_next_date": "2026-08-22",
            "last_date": "2026-07-22",
            "status": "MATURE",
            "is_active": true,
            "personal_finance_category": {
                "primary": "TRANSFER_OUT",
                "detailed": "TRANSFER_OUT_ACCOUNT_TRANSFER"
            }
        }],
        "inflow_streams": [{
            "stream_id": "stream-2",
            "account_id": "acct-1",
            "description": "INTRST PYMNT",
            "frequency": "MONTHLY",
            "average_amount": { "amount": -4.22 },
            "predicted_next_date": "2026-09-07",
            "is_active": true,
            "personal_finance_category": {
                "primary": "TRANSFER_IN",
                "detailed": "TRANSFER_IN_INTEREST_EARNED"
            }
        }]
    }"#;

    #[test]
    fn parses_streams() {
        let res: RecurringResponse = serde_json::from_str(BODY).unwrap();

        assert_eq!(res.outflow_streams.len(), 1);
        assert_eq!(res.inflow_streams.len(), 1);

        let out = &res.outflow_streams[0];
        assert_eq!(out.frequency, "MONTHLY");
        assert_eq!(out.average_amount.amount, Some(2078.5));
        assert_eq!(out.predicted_next_date, Some(1_787_356_800));
        assert_eq!(out.is_active, Some(true));

        // Plaid sends "" rather than null for an unmatched merchant.
        assert_eq!(out.merchant_name.as_deref(), Some(""));
    }

    /// A stream Plaid gave up predicting still has to parse.
    #[test]
    fn a_stream_without_a_prediction_parses() {
        let body = r#"{"outflow_streams":[{
            "stream_id": "s", "account_id": "a", "description": "d",
            "frequency": "UNKNOWN", "average_amount": {"amount": 1.0}
        }]}"#;

        let res: RecurringResponse = serde_json::from_str(body).unwrap();

        assert_eq!(res.outflow_streams[0].predicted_next_date, None);
        assert_eq!(res.outflow_streams[0].is_active, None);
    }
}
