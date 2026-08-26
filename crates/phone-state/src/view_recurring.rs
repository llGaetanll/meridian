//! The Recurring screen: what comes back.

use crate::ViewCalendar;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewRecurring {
    /// `"Every month"` — what the headline under it is a figure for.
    pub headline_label: String,

    /// `"−$3,767.18"`, the net of everything that comes back.
    pub headline: String,

    /// `"+$4.22 in · net −$3,767.18"`.
    ///
    /// The phone draws `flow_in` and `flow_out` instead; this is the one line
    /// the web still sets under its headline.
    pub detail: String,

    /// `"$4.22"` and `"$3,771.40"` — the two directions, unsigned. The words
    /// beside them say which way the money goes, so the figures do not carry a
    /// sign as well.
    pub flow_in: String,
    pub flow_out: String,

    /// `"+$4.22"` and `"−$3,771.40"` — the same two with the sign back on, for
    /// where there is no room for the words. A diff, read as one.
    pub flow_in_signed: String,
    pub flow_out_signed: String,

    pub masked: bool,

    /// `"Where it goes"` and `"−$3,771.40"` — the heading over the split.
    pub breakdown_label: String,
    pub breakdown_total: String,

    /// The breakdown bar and its legend are two views of one split, so they
    /// come from one list.
    pub breakdown: Vec<ViewBreakdownSlice>,

    /// When it all lands, under the breakdown.
    pub calendar: ViewCalendar,

    /// What landed on the day that was tapped. `None` until one is.
    pub day_card: Option<ViewDayCard>,

    /// `"Every stream"` and `"8"` — the heading over the list.
    pub streams_label: String,
    pub streams_count: String,

    pub groups: Vec<ViewStreamGroup>,
}

/// One segment of the breakdown bar.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewBreakdownSlice {
    pub label: String,
    pub token: String,

    /// The segment's share of the bar, 0..1.
    ///
    /// A ratio rather than a percentage string, because this one is geometry —
    /// the exception `docs/FFI.md` carves out for the chart.
    pub share: f64,

    /// `"$2,078.50"`.
    pub amount: String,
}

/// The streams landing on the day the calendar has selected.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewDayCard {
    /// `"Fri, Jul 10"`.
    pub heading: String,

    /// `"−$578.50"`, what the day comes to.
    pub total: String,

    pub rows: Vec<ViewDayRow>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewDayRow {
    pub id: String,
    /// `"United Airlines"`.
    pub name: String,
    /// `"−$500.00"`.
    pub amount: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewStreamGroup {
    /// `"Transfer out"`.
    pub name: String,
    pub token: String,
    /// `"$2,078.50"`.
    pub total: String,
    /// `"1 stream"` / `"2 streams"`, which is what the group's heading says
    /// now that the figures below it are the ones worth reading.
    pub count_label: String,
    pub streams: Vec<ViewStreamRow>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewStreamRow {
    pub id: String,
    pub title: String,
    pub logo_url: Option<String>,

    /// `"next Aug 22 · in 3 days"`.
    pub when: String,

    /// `"Monthly · next Aug 22"` — the whole line under the name, which is
    /// the frequency and the date read as one phrase rather than two.
    pub sub: String,

    /// Due within the week, so it reads as a heads-up rather than an alarm.
    pub soon: bool,

    /// `"−$2,078.50"`.
    pub amount: String,

    /// `"Monthly"`.
    pub frequency: String,
}
