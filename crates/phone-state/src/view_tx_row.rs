//! One transaction, as a row.

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewTxRow {
    /// Stable across refreshes, so a platform list can diff on it.
    pub id: String,

    /// The merchant, or the raw bank description where there is no merchant.
    pub title: String,

    /// What the row calls its category, e.g. `"Food & drink"`.
    pub category: String,

    /// A palette token for the category, e.g. `"c-food"`. The view maps this
    /// to its own colour; it never works one out from the category name.
    pub category_token: String,

    /// The amount, ready to print: `"−$5.40"`, `"+$500.00"`.
    pub amount: String,

    /// Whether the amount should read as money arriving. The view uses this
    /// for colour; it does not parse the sign back out of `amount`.
    pub is_inflow: bool,

    /// A merchant logo, where one exists. The slot is reserved either way, so
    /// every row's text starts on the same line.
    pub logo_url: Option<String>,

    /// Authorised but not settled. The whole row recedes.
    pub pending: bool,
}
