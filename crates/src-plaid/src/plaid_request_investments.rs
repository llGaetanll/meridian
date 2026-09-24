use api_common::Timestamp;
use serde_json::Value;

use crate::PlaidRequest;

/// `/investments/transactions/get` — one page of the trades between two
/// dates.
///
/// Not a sync: there is no cursor, only a date window and an offset into it,
/// and Plaid holds at most 24 months. The answer says how many transactions
/// the window holds in total, and pages past the first are the caller's to
/// ask for.
pub struct InvestmentsRequest {
    pub access_token: String,

    /// The earliest day asked for, inclusive.
    pub start_date: Timestamp,

    /// The latest day asked for, inclusive.
    pub end_date: Timestamp,

    /// How many to return per page, capped by Plaid at 500.
    pub count: u32,

    /// How many transactions into the window this page starts.
    pub offset: u32,
}

impl InvestmentsRequest {
    /// The first page of the window.
    pub fn new(
        access_token: impl Into<String>,
        start_date: Timestamp,
        end_date: Timestamp,
    ) -> Self {
        Self::page(access_token, start_date, end_date, 0)
    }

    /// A later page of the same window, starting `offset` transactions in.
    pub fn page(
        access_token: impl Into<String>,
        start_date: Timestamp,
        end_date: Timestamp,
        offset: u32,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            start_date,
            end_date,
            count: 500,
            offset,
        }
    }
}

impl PlaidRequest for InvestmentsRequest {
    fn path(&self) -> String {
        String::from("investments/transactions/get")
    }

    fn body(&self) -> Value {
        serde_json::json!({
            "access_token": self.access_token,
            "start_date": iso_date(self.start_date),
            "end_date": iso_date(self.end_date),
            "options": { "count": self.count, "offset": self.offset },
        })
    }
}

/// `2026-08-14`, which is the only date format Plaid takes. The mirror of the
/// parsing in `plaid_transaction`, so a `Timestamp` holds the date everywhere
/// but on the wire.
fn iso_date(ts: Timestamp) -> String {
    let secs = i64::try_from(ts.as_secs()).unwrap_or(0);

    match chrono::DateTime::from_timestamp(secs, 0) {
        Some(dt) => dt.format("%Y-%m-%d").to_string(),
        None => String::from("1970-01-01"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-08-14, midday — the date must survive not being midnight.
    const DAY: Timestamp = Timestamp::from_secs(1_786_708_800);

    #[test]
    fn dates_go_out_in_plaids_format() {
        let body = InvestmentsRequest::new("access-sandbox-1", Timestamp::EPOCH, DAY).body();

        assert_eq!(body["start_date"], "1970-01-01");
        assert_eq!(body["end_date"], "2026-08-14");
    }

    #[test]
    fn the_first_page_starts_at_zero() {
        let body = InvestmentsRequest::new("access-sandbox-1", Timestamp::EPOCH, DAY).body();

        assert_eq!(body["options"]["offset"], 0);
        assert_eq!(body["options"]["count"], 500);
    }

    #[test]
    fn later_pages_carry_their_offset() {
        let body = InvestmentsRequest::page("access-sandbox-1", Timestamp::EPOCH, DAY, 500).body();

        assert_eq!(body["options"]["offset"], 500);
    }
}
