//! The shape Plaid answers `/transactions/sync` with.
//!
//! Plaid's types as Plaid reports them — the sign of `amount` and the pending
//! duplicates are noted where they appear, but neither is resolved here. That
//! belongs to whatever reads these into the ledger; `docs/PLAID.md` has both.

use api_common::Timestamp;
use serde::Deserialize;

/// One transaction, as Plaid reports it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaidTransaction {
    pub transaction_id: String,
    pub account_id: String,

    /// **Positive means money left the account.** Inverted from what a ledger
    /// wants, and inverting a whole spending summary is a quiet way to be
    /// wrong, so whatever reads this into the ledger negates it. See
    /// `docs/PLAID.md`.
    pub amount: f64,

    /// ISO 4217, e.g. `USD`. Optional in the API; some accounts report only
    /// `unofficial_currency_code`.
    #[serde(default)]
    pub iso_currency_code: Option<String>,

    /// When the transaction posted.
    #[serde(deserialize_with = "date")]
    pub date: Timestamp,

    /// When it was authorised, which is usually what the user remembers. Not
    /// always present.
    #[serde(default, deserialize_with = "optional_date")]
    pub authorized_date: Option<Timestamp>,

    /// Raw description off the bank feed.
    pub name: String,

    /// Cleaned-up merchant, when Plaid recognises one. This is the one worth
    /// showing.
    #[serde(default)]
    pub merchant_name: Option<String>,

    /// The merchant's logo, where Plaid has one. Only about half of a real
    /// feed carries this, so the view reserves the slot and shows nothing
    /// rather than substituting a glyph.
    #[serde(default)]
    pub logo_url: Option<String>,

    /// Still pending. A pending transaction is *replaced* by a posted one with
    /// a different id — see `pending_transaction_id`.
    pub pending: bool,

    /// On a posted transaction, the id of the pending one it replaces. Ignore
    /// this and the same purchase is counted twice.
    #[serde(default)]
    pub pending_transaction_id: Option<String>,

    /// Plaid's own categorisation, which is what saves us writing a
    /// classifier for the MVP.
    #[serde(default, rename = "personal_finance_category")]
    pub category: Option<Category>,
}

impl PlaidTransaction {
    /// What to show for this transaction: the merchant when Plaid worked one
    /// out, and the raw bank description when it did not.
    pub fn label(&self) -> &str {
        self.merchant_name.as_deref().unwrap_or(&self.name)
    }
}

/// Plaid's personal finance category. `primary` is the coarse bucket
/// (`FOOD_AND_DRINK`), `detailed` the finer one (`FOOD_AND_DRINK_COFFEE`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Category {
    pub primary: String,
    pub detailed: String,
}

/// Plaid sends `"2026-08-14"`, and the ledger counts seconds. Parsed at the
/// edge so nothing downstream has to know Plaid's format.
///
/// A date has no time in it, so this is midnight UTC. Good enough to group by
/// day, which is all the dates are used for; a transaction's real instant is
/// not something Plaid reports anyway.
fn parse_date(s: &str) -> Option<Timestamp> {
    let [y, m, d]: [&str; 3] = s.split('-').collect::<Vec<_>>().try_into().ok()?;

    let (y, m, d) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);

    let date = chrono::NaiveDate::from_ymd_opt(y, m, d)?;

    let secs = date.and_hms_opt(0, 0, 0)?.and_utc().timestamp();

    // Plaid does not report dates before 1970, and a negative one would wrap
    // to an absurd future date rather than failing.
    u64::try_from(secs).ok().map(Timestamp::from_secs)
}

pub(crate) fn date<'de, D>(deserializer: D) -> Result<Timestamp, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    parse_date(&s).ok_or_else(|| serde::de::Error::custom(format!("bad date: {s}")))
}

pub(crate) fn optional_date<'de, D>(deserializer: D) -> Result<Option<Timestamp>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(s) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };

    parse_date(&s)
        .map(Some)
        .ok_or_else(|| serde::de::Error::custom(format!("bad date: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_date() {
        assert_eq!(
            parse_date("2026-08-14"),
            Some(Timestamp::from_secs(1_786_665_600))
        );
        assert_eq!(parse_date("1970-01-01"), Some(Timestamp::EPOCH));
    }

    #[test]
    fn rejects_nonsense() {
        assert_eq!(parse_date("14/08/2026"), None);
        assert_eq!(parse_date("2026-13-01"), None);
        assert_eq!(parse_date(""), None);
    }
}
