//! The Ledger screen: everything that has happened, newest first.

use crate::ViewFilter;
use crate::ViewTxRow;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewLedger {
    /// `"3 connections · synced 2m ago"`.

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
    pub days: Vec<ViewDaySection>,
}

/// A day's worth of rows, under its heading.
///
/// Grouping is done in the machine rather than in the view because three view
/// layers would otherwise each write the same date-bucketing loop, and the one
/// that got it subtly wrong would be the one nobody looked at.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewDaySection {
    /// `"Wed, Aug 12"`.
    pub heading: String,
    pub rows: Vec<ViewTxRow>,
}
