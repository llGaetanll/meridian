//! The month, and which days money leaves on it.

/// Three months side by side: the one you are looking at, and the one either
/// side of it.
///
/// Turning the calendar is a scroll rather than a redraw, so the neighbouring
/// months are here in full rather than as the one visible column.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendar {
    /// `["M", "T", "W", "T", "F", "S", "S"]` — the column headings, starting
    /// on Monday. One set for all three months.
    pub weekdays: Vec<String>,

    pub prev: ViewCalendarMonth,
    pub current: ViewCalendarMonth,
    pub next: ViewCalendarMonth,
}

/// One month's grid, with as many rows as the month has weeks in it — five
/// for July, six for August — so the grid changes height as the months turn.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendarMonth {
    /// `"August 2026"`. The year on all three months, because the label
    /// travels from the edge to the centre as the month turns.
    pub label: String,

    pub weeks: Vec<Vec<ViewCalendarDay>>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendarDay {
    /// What a tap sends back: the day itself, not where it sits in the grid,
    /// so the same day in two months' grids is one day.
    pub id: String,

    /// `"12"`.
    pub label: String,

    /// A day belonging to the month either side. Drawn, and dimmed.
    pub outside: bool,

    pub today: bool,

    /// The day the user tapped, filled in grey.
    pub picked: bool,

    /// One per stream landing that day, as category tokens. Only dates the
    /// source predicts — nothing is projected forward from a frequency.
    pub marks: Vec<String>,
}
