//! One transaction, as a row.

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewTxRow {
    /// Stable across refreshes, so a platform list can diff on it.
    pub id: String,

    /// The merchant, or the raw bank description where there is no merchant.
    pub title: String,

    /// What the row calls its category, e.g. `"Food & drink"`.
    pub category: String,

    /// A palette token for the category, e.g. `"c-food"`, which the view maps
    /// to its own colour.
    pub category_token: String,

    /// The amount, ready to print: `"−$5.40"`, `"+$500.00"`.
    pub amount: String,

    /// Whether the amount reads as money arriving, so that nothing has to
    /// parse the sign back out of `amount`.
    pub is_inflow: bool,

    /// A merchant logo, where one exists. The slot is reserved either way.
    pub logo_url: Option<String>,

    /// Authorised but not settled.
    pub pending: bool,
}
