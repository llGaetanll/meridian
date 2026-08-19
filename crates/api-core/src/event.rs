//! What happened. See `docs/ARCHITECTURE.md` — events go in, actions come out,
//! and nothing in between touches the outside world.

use crate::view::Screen;

/// Where an event came from. Grouping by origin is what keeps a match on
/// `Event` readable as the app grows, and makes a recorded log read as a story.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Event {
    FromUi { event: UiEvent },
    FromSource { event: SourceEvent },
}

/// The user did something.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum UiEvent {
    /// The app came up. Carries the clock, because the machine has none.
    Started { now: u64 },

    /// A tab in the dock.
    ScreenOpened { screen: Screen },

    /// Pull to refresh.
    RefreshRequested,

    /// The eye on a covered figure. One switch for the whole app: revealing on
    /// Balances and staying covered on Home would be a strange thing to have
    /// to explain.
    MaskToggled,

    /// The filter panel as a whole.
    FilterToggled,

    /// One of the two groups inside it.
    FilterGroupToggled { group: FilterGroup },

    /// A category chip. Multi-select, so this toggles rather than sets.
    CategoryToggled { id: String },

    /// Clear every category.
    CategoriesCleared,

    /// One of the four range forms.
    RangeSelected { kind: RangeSelection },

    /// The user picked a transaction out of the list.
    TxSelected { id: String },
}

/// Which half of the filter panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FilterGroup {
    Categories,
    Range,
}

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
    /// Between two days, as unix timestamps.
    Between { start: u64, end: u64 },
    /// Everything.
    All,
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

/// A source answered, or failed to.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum SourceEvent {
    /// A snapshot arrived. Everything the app knows, in one value — see
    /// `api-wire`.
    ///
    /// The payload is JSON rather than a structured record because the
    /// alternative is redeclaring every wire type as a UniFFI record and
    /// keeping the two in step by hand. The core parses it, which keeps the
    /// parsing in one place for all three platforms; `docs/DECISIONS.md` has
    /// the longer argument.
    SnapshotLoaded { json: String, now: u64 },

    /// A request did not come back.
    LoadFailed { message: String },
}
