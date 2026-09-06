//! The Ledger screen: everything that has happened, newest first.

use crate::ViewFilter;
use crate::ViewTxRow;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewLedger {
    /// The transactions, grouped by month and then by day.
    pub months: Vec<ViewMonthSection>,

    /// The filter panel, whose state travels with the screen it filters.
    pub filter: ViewFilter,

    /// What to show instead of the list when it is empty. `None` means there
    /// are rows.
    pub empty: Option<String>,
}

/// A month's worth of days, under its rule.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewMonthSection {
    /// `"August"`.
    pub name: String,
    /// `"2026"`.
    pub year: String,
    /// `"August 2026"` — the two of them as the marker reads them.
    pub label: String,
    pub days: Vec<ViewDaySection>,
}

/// A day's worth of rows, under its heading.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewDaySection {
    /// `"Wed, Aug 12"`.
    pub heading: String,

    /// `"−$30.40"`, the day's net.
    pub total: String,

    pub rows: Vec<ViewTxRow>,
}
