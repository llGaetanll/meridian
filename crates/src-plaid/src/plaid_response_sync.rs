//! What `/transactions/sync` answers with.

use serde::Deserialize;

use crate::PlaidTransaction;

/// One page of changes.
///
/// Three lists rather than one, because a sync is a diff and not a snapshot.
/// `removed` matters as much as `added`: a transaction the bank retracted has
/// to leave the ledger, and nothing else says it should.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SyncResponse {
    #[serde(default)]
    pub added: Vec<PlaidTransaction>,

    #[serde(default)]
    pub modified: Vec<PlaidTransaction>,

    /// Only ids come back for these — the transaction is gone, so there is
    /// nothing else to send.
    #[serde(default)]
    pub removed: Vec<RemovedTransaction>,

    /// Where to resume. Saved and passed to the next call.
    pub next_cursor: String,

    /// Whether another page is waiting. Plaid pages at `count`, so a first
    /// sync of a real account is several calls, each with the previous one's
    /// `next_cursor`.
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemovedTransaction {
    pub transaction_id: String,
}

#[cfg(test)]
mod tests {
    use api_common::Timestamp;

    use super::*;

    /// Shaped like a real answer, trimmed to the fields we read.
    const BODY: &str = r#"{
        "added": [{
            "transaction_id": "tx-1",
            "account_id": "acct-1",
            "amount": 12.34,
            "iso_currency_code": "USD",
            "date": "2026-08-14",
            "authorized_date": "2026-08-13",
            "name": "SQ *BLUE BOTTLE COFFEE",
            "merchant_name": "Blue Bottle Coffee",
            "pending": false,
            "pending_transaction_id": "tx-0",
            "personal_finance_category": {
                "primary": "FOOD_AND_DRINK",
                "detailed": "FOOD_AND_DRINK_COFFEE"
            }
        }],
        "modified": [],
        "removed": [{"transaction_id": "tx-9"}],
        "next_cursor": "cursor-abc",
        "has_more": false
    }"#;

    #[test]
    fn parses_a_sync_page() {
        let res: SyncResponse = serde_json::from_str(BODY).unwrap();

        assert_eq!(res.next_cursor, "cursor-abc");
        assert!(!res.has_more);
        assert_eq!(res.removed[0].transaction_id, "tx-9");

        let tx = &res.added[0];
        assert_eq!(tx.transaction_id, "tx-1");
        assert_eq!(tx.amount, 12.34);
        assert_eq!(tx.merchant_name.as_deref(), Some("Blue Bottle Coffee"));
        assert_eq!(tx.pending_transaction_id.as_deref(), Some("tx-0"));

        let category = tx.category.as_ref().unwrap();
        assert_eq!(category.primary, "FOOD_AND_DRINK");
        assert_eq!(category.detailed, "FOOD_AND_DRINK_COFFEE");
    }

    /// Plaid sends dates, not epochs. 2026-08-14 UTC midnight.
    #[test]
    fn dates_become_timestamps() {
        let res: SyncResponse = serde_json::from_str(BODY).unwrap();

        assert_eq!(res.added[0].date, Timestamp::from_secs(1_786_665_600));
        assert_eq!(
            res.added[0].authorized_date,
            Some(Timestamp::from_secs(1_786_579_200))
        );
    }

    /// A cash account reports no `merchant_name` and no category, and the
    /// optional fields are absent rather than null.
    #[test]
    fn parses_a_sparse_transaction() {
        let body = r#"{
            "added": [{
                "transaction_id": "tx-2",
                "account_id": "acct-1",
                "amount": -500.0,
                "date": "2026-08-01",
                "name": "DIRECT DEP PAYROLL",
                "pending": false
            }],
            "next_cursor": "cursor-def",
            "has_more": false
        }"#;

        let res: SyncResponse = serde_json::from_str(body).unwrap();
        let tx = &res.added[0];

        assert_eq!(tx.merchant_name, None);
        assert_eq!(tx.category, None);
        assert_eq!(tx.authorized_date, None);
        assert_eq!(tx.iso_currency_code, None);
        assert!(res.modified.is_empty());
        assert!(res.removed.is_empty());
    }
}
