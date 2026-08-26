//! The filter panel: categories and a time range, independent of each other.

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewFilter {
    pub categories_open: bool,
    pub range_open: bool,

    /// All sixteen, each with its live count.
    pub categories: Vec<ViewCategoryChip>,

    /// What a shut category group says instead of its chips: the first few
    /// selected, then `"+2"`.
    pub categories_summary: Vec<ViewCategoryChip>,
    pub categories_overflow: Option<String>,

    pub range: ViewRange,

    /// `"Last 3 months"` — what a shut range group shows.
    pub range_summary: String,
}

/// One category in the filter grid.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewCategoryChip {
    /// The stable name, e.g. `"FoodAndDrink"` — what a tap sends back.
    pub id: String,
    /// `"Food & drink"`.
    pub label: String,
    /// `"c-food"`.
    pub token: String,
    /// How many transactions match, as text.
    pub count: String,
    pub selected: bool,
    /// Nothing matches. The chip stays visible but reads as unavailable.
    pub empty: bool,
}

/// The time range, as the sentence the sketch draws.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewRange {
    /// Which of the four forms is live.
    pub kind: ViewRangeKind,

    /// "This [month]" — the unit token.
    pub this_unit: String,

    /// "Last [3] [months]" — the count and unit tokens.
    pub last_count: String,
    pub last_unit: String,

    /// "Between [Jun 1] and [Aug 19]".
    pub between_start: String,
    pub between_end: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ViewRangeKind {
    This,
    Last,
    Between,
    All,
}
