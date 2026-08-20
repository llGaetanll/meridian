//! The time range, as the user picks it.

use api_common::Timestamp;

/// A time range the user asked for.
///
/// The parameters ride along with the choice rather than being separate
/// events, so "last 3 months" is one event and not three. A replay of the log
/// then shows the range the user actually picked, not a reconstruction of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RangeSelection {
    /// This month, this week, this year.
    This { unit: RangeUnit },
    /// The last N of them.
    Last { count: u32, unit: RangeUnit },
    /// Between two days, as seconds since the epoch.
    ///
    /// A bare `u64` rather than a [`Timestamp`], because this crosses FFI and
    /// `custom_newtype!` cannot be applied to a type from another crate. The
    /// conversion happens in [`RangeSelection::window_bounds`], so nothing
    /// downstream handles a raw integer.
    Between { start: u64, end: u64 },
    /// Everything.
    All,
}

impl RangeSelection {
    /// The two ends of a `Between`, as timestamps.
    pub fn window_bounds(&self) -> Option<(Timestamp, Timestamp)> {
        match self {
            RangeSelection::Between { start, end } => {
                Some((Timestamp::from_secs(*start), Timestamp::from_secs(*end)))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RangeUnit {
    Week,
    Month,
    Year,
}

impl RangeUnit {
    /// Singular or plural, as the sentence needs it.
    pub fn label(&self, count: u32) -> &'static str {
        match (self, count) {
            (RangeUnit::Week, 1) => "week",
            (RangeUnit::Week, _) => "weeks",
            (RangeUnit::Month, 1) => "month",
            (RangeUnit::Month, _) => "months",
            (RangeUnit::Year, 1) => "year",
            (RangeUnit::Year, _) => "years",
        }
    }

    /// How many seconds one of these covers.
    ///
    /// Approximate for months and years on purpose: this is used to find the
    /// start of a "last 3 months" window, where the user means "about three
    /// months back" and not a calendar-exact boundary. The `This` forms, which
    /// do mean an exact boundary, are computed from the civil date instead.
    pub fn seconds(&self) -> u64 {
        match self {
            RangeUnit::Week => 7 * 86_400,
            RangeUnit::Month => 30 * 86_400,
            RangeUnit::Year => 365 * 86_400,
        }
    }
}
