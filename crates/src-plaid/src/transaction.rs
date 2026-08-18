//! The shape Plaid answers `/transactions/sync` with, and the two things about
//! it that are easy to get wrong.
//!
//! Written from the documented API rather than from a live response, so treat
//! the field set as approximate until something has actually been fetched.

use api_common::Timestamp;

/// One transaction, as Plaid reports it.
#[derive(Debug, Clone)]
pub struct PlaidTransaction {
    pub transaction_id: String,
    pub account_id: String,

    /// **Positive means money left the account.** Inverted from what a ledger
    /// wants, and inverting a whole spending summary is a quiet way to be
    /// wrong, so the conversion into `api_ledger` is the place to be careful.
    pub amount: f64,

    /// ISO 4217, e.g. `USD`. Optional in the API; some accounts report only
    /// `unofficial_currency_code`.
    pub iso_currency_code: Option<String>,

    /// When the transaction posted.
    pub date: Timestamp,

    /// When it was authorised, which is usually what the user remembers. Not
    /// always present.
    pub authorized_date: Option<Timestamp>,

    /// Raw description off the bank feed.
    pub name: String,

    /// Cleaned-up merchant, when Plaid recognises one. This is the one worth
    /// showing.
    pub merchant_name: Option<String>,

    /// Still pending. A pending transaction is *replaced* by a posted one with
    /// a different id — see `pending_transaction_id`.
    pub pending: bool,

    /// On a posted transaction, the id of the pending one it replaces. Ignore
    /// this and the same purchase is counted twice.
    pub pending_transaction_id: Option<String>,

    /// Plaid's own categorisation, which is what saves us writing a
    /// classifier for the MVP.
    pub category: Option<Category>,
}

/// Plaid's personal finance category. `primary` is the coarse bucket
/// (`FOOD_AND_DRINK`), `detailed` the finer one (`FOOD_AND_DRINK_COFFEE`).
#[derive(Debug, Clone)]
pub struct Category {
    pub primary: String,
    pub detailed: String,
}
