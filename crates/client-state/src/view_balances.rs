//! The Balances screen: where the money sits.

use crate::ViewBreakdownSlice;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewBalances {
    /// `"Held"`. What you hold, not net worth: Plaid reports the mortgage but
    /// never the house behind it, so a net figure is structurally too red.
    pub headline_label: String,

    /// `"$86,541.74"`, or the same figure covered.
    pub headline: String,

    /// Whether the figures are covered. They are already replaced in the text
    /// above, so nothing is merely hidden in the DOM.
    pub masked: bool,

    /// `"Cover figures"` or `"Show figures"` — what the eye does when pressed.
    pub mask_label: String,

    /// `"Owed"` and `"−$3,090.95"` — the heading over the debt blocks.
    pub owed_label: String,
    pub owed_total: String,

    pub connections: Vec<ViewConnection>,

    /// `"Where it sits"` — the heading over how what is held splits across
    /// the banks holding it.
    pub held_split_label: String,

    /// One slice per bank holding anything, largest first. The label is the
    /// bank's name and the token its connection id.
    pub held_split: Vec<ViewBreakdownSlice>,

    /// `"Owed against it"` — the same split, over what is owed.
    pub owed_split_label: String,

    pub owed_split: Vec<ViewBreakdownSlice>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewConnection {
    pub id: String,
    pub name: String,
    pub initials: String,

    /// `"synced 2m"`, or empty when the connection needs attention.
    pub synced: String,

    /// Whether the user has to sign in again.
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

    /// `"Checking ·0000"` — the first half of the subtitle, for where it gets
    /// a column of its own.
    pub kind: String,

    /// `"$100.00 available"` or `"4.10% APY"` — the rest of it, or empty.
    pub detail: String,

    /// `"$110.00"` or `"−$56,302.06"`.
    pub amount: String,

    /// Whether this is money owed, for colour.
    pub owed: bool,

    /// `"$410 of $2,000 limit used"`, where a limit exists.
    pub utilisation: Option<String>,
}
