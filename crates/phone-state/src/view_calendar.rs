//! The month, and which days money leaves on it.

/// A calendar of the month the app is in.
///
/// Six rows always, and the days either side of the month drawn rather than
/// left blank — a grid that changes height as the months turn moves everything
/// under it, and a row of empty cells reads as a rendering fault.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendar {
    /// `"August 2026"`.
    pub month: String,

    /// `["M", "T", "W", "T", "F", "S", "S"]` — the column headings, starting
    /// on Monday.
    pub weekdays: Vec<String>,

    pub weeks: Vec<Vec<ViewCalendarDay>>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendarDay {
    /// `"12"`.
    pub label: String,

    /// A day belonging to the month either side. Drawn, and dimmed.
    pub outside: bool,

    pub today: bool,

    /// One per stream landing that day, as category tokens — the same tokens
    /// the breakdown above colours its segments with.
    ///
    /// Only dates the source actually predicts. Nothing here is projected
    /// forward from a stream's frequency, so a month shows as many marks as
    /// there are streams and no more.
    pub marks: Vec<String>,
}
