//! The Balances screen: where the money sits.

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewBalances {
    /// `"3 connections · 14 accounts"`.

    /// `"Held"`. The headline is what you hold, not net worth: Plaid reports
    /// the mortgage but never the house behind it, so a net figure is
    /// structurally too red.
    pub headline_label: String,

    /// `"$86,541.74"`, or the same figure covered.
    pub headline: String,

    /// Whether the figures are covered. The view draws a bar over them; the
    /// value itself was already replaced, so nothing is hidden in the DOM.
    pub masked: bool,

    pub connections: Vec<ViewConnection>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewConnection {
    pub id: String,
    pub name: String,
    pub initials: String,

    /// `"synced 2m"`, or empty when the connection needs attention.
    pub synced: String,

    /// Whether the user has to sign in again. Carries the action rather than
    /// just naming the problem.
    pub needs_reconnect: bool,

    /// Whether these balances are owed rather than held.
    pub is_debts: bool,

    pub accounts: Vec<ViewAccountRow>,

    /// `"6 accounts held"` / `"6 accounts owed"`.
    pub total_label: String,
    /// `"$62,589.00"`.
    pub total: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccountRow {
    pub id: String,
    pub name: String,

    /// `"Checking ·0000 · $100.00 available"` — the whole subtitle, assembled.
    pub subtitle: String,

    /// `"$110.00"` or `"−$56,302.06"`.
    pub amount: String,

    /// Whether this is money owed, for colour.
    pub owed: bool,

    /// `"$410 of $2,000 limit used"`, where a limit exists.
    pub utilisation: Option<String>,
}
