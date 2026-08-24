//! The Recurring screen: what comes back.

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewRecurring {
    /// `"8 streams · 7 out, 1 in"`.

    /// `"−$3,771.40"`, the monthly commitment.
    pub headline: String,

    /// `"+$4.22 in · net −$3,767.18"`.
    pub detail: String,

    pub masked: bool,

    /// The breakdown bar and its legend are two views of one split, so they
    /// come from one list.
    pub breakdown: Vec<ViewBreakdownSlice>,

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

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewStreamGroup {
    /// `"Transfer out"`.
    pub name: String,
    pub token: String,
    /// `"$2,078.50"`.
    pub total: String,
    pub streams: Vec<ViewStreamRow>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewStreamRow {
    pub id: String,
    pub title: String,
    pub logo_url: Option<String>,

    /// `"next Aug 22 · in 3 days"`.
    pub when: String,

    /// Due within the week, so it reads as a heads-up rather than an alarm.
    pub soon: bool,

    /// `"−$2,078.50"`.
    pub amount: String,

    /// `"Monthly"`.
    pub frequency: String,
}
