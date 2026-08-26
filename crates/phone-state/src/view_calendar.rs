//! The month, and which days money leaves on it.

/// Three months side by side: the one you are looking at, and the one either
/// side of it.
///
/// The calendar runs sideways, because months do, and turning it is a scroll
/// rather than a redraw — so the months either side have to be here in full.
/// Only a column of each shows at the edge, but that column is the real month's
/// first or last, and a swipe brings the rest of it over.
///
/// Drawn as a whole month each rather than as the one visible column, because a
/// column that has to become a month when the finger moves is two drawings of
/// the same thing that can disagree.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendar {
    /// `["M", "T", "W", "T", "F", "S", "S"]` — the column headings, starting
    /// on Monday. One set: all three months are laid out the same way.
    pub weekdays: Vec<String>,

    pub prev: ViewCalendarMonth,
    pub current: ViewCalendarMonth,
    pub next: ViewCalendarMonth,
}

/// One month's grid.
///
/// As many rows as the month has weeks in it — five for July, six for August —
/// rather than six always. A row that is entirely next month is a row of
/// nothing, and the grid changing height as the months turn is the price.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendarMonth {
    /// `"August 2026"`.
    ///
    /// The year on all three and not just the middle one: the label travels
    /// from the edge to the centre as the month turns, and a label that gains a
    /// year on the way is a label that changes rather than moves.
    pub label: String,

    pub weeks: Vec<Vec<ViewCalendarDay>>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCalendarDay {
    /// What a tap on this day sends back — the day itself, not where it sits
    /// in the grid, so the same day in two months' grids is one day.
    pub id: String,

    /// `"12"`.
    pub label: String,

    /// A day belonging to the month either side. Drawn, and dimmed.
    pub outside: bool,

    pub today: bool,

    /// The day the user tapped, filled in grey. Grey and not outlined: an
    /// outline reads as a different kind of control sitting in the grid.
    pub picked: bool,

    /// One per stream landing that day, as category tokens — the same tokens
    /// the breakdown above colours its segments with.
    ///
    /// Only dates the source actually predicts. Nothing here is projected
    /// forward from a stream's frequency, so a month shows as many marks as
    /// there are streams and no more.
    pub marks: Vec<String>,
}
