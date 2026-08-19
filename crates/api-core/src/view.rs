//! What a screen shows, already decided.
//!
//! Every type here carries finished values. A `TxRow` holds the date as the
//! string the row prints and the amount as the text `"−$5.40"`, not a
//! timestamp and an `f64` for three platforms to format the same way three
//! times. `docs/FFI.md` is explicit about why: the type surface gets a little
//! larger, and the *logic* surface — the part that can disagree between
//! platforms — stays at zero.
//!
//! The rule to hold onto when adding to this file: if a view layer would have
//! to write an `if`, a `switch`, or a `format!` to use a field, the field is
//! wrong. Send the answer, not the ingredients.

/// Which screen the app is on.
///
/// The dock's four tabs. Search is deliberately absent — the sketch draws the
/// button but never the screen, so there is nothing to be faithful to yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum Screen {
    #[default]
    Home,
    Ledger,
    Recurring,
    Balances,
}

/// One transaction, as a row.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TxRow {
    /// Stable across refreshes, so a platform list can diff on it.
    pub id: String,

    /// The merchant, or the raw bank description where there is no merchant.
    pub title: String,

    /// What the row calls its category, e.g. `"Food & drink"`.
    pub category: String,

    /// A palette token for the category, e.g. `"c-food"`. The view maps this
    /// to its own colour; it never works one out from the category name.
    pub category_token: String,

    /// The amount, ready to print: `"−$5.40"`, `"+$500.00"`.
    pub amount: String,

    /// Whether the amount should read as money arriving. The view uses this
    /// for colour; it does not parse the sign back out of `amount`.
    pub is_inflow: bool,

    /// A merchant logo, where one exists. The slot is reserved either way, so
    /// every row's text starts on the same line.
    pub logo_url: Option<String>,

    /// Authorised but not settled. The whole row recedes.
    pub pending: bool,
}

/// A day's worth of rows, under its heading.
///
/// Grouping is done here rather than in the view because three view layers
/// would otherwise each write the same date-bucketing loop, and the one that
/// got it subtly wrong would be the one nobody looked at.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DaySection {
    /// `"Wed, Aug 12"`.
    pub heading: String,
    pub rows: Vec<TxRow>,
}

/// A month's worth of days, under its rule.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MonthSection {
    /// `"August"`.
    pub name: String,
    /// `"2026"`.
    pub year: String,
    pub days: Vec<DaySection>,
}

/// The Ledger screen.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LedgerView {
    /// `"3 connections · synced 2m ago"`.
    pub subtitle: String,

    pub months: Vec<MonthSection>,

    /// The filter panel, whose state travels with the screen it filters.
    pub filter: FilterView,

    /// What to show instead of the list when it is empty, if it is empty.
    /// `None` means there are rows.
    pub empty: Option<String>,
}

/// The filter panel: categories and a time range, independent of each other.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FilterView {
    pub open: bool,

    pub categories_open: bool,
    pub range_open: bool,

    /// All sixteen, each with its live count.
    pub categories: Vec<CategoryChip>,

    /// What a shut category group says instead of its chips: the first few
    /// selected, then `"+2"`.
    pub categories_summary: Vec<CategoryChip>,
    pub categories_overflow: Option<String>,

    pub range: RangeView,

    /// `"Last 3 months"` — what a shut range group shows.
    pub range_summary: String,

    /// Whether anything is filtering at all. Drives the dot on the filter
    /// button.
    pub active: bool,
}

/// One category in the filter grid, or on a row.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CategoryChip {
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
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RangeView {
    /// Which of the four forms is live.
    pub kind: RangeKind,

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
pub enum RangeKind {
    This,
    Last,
    Between,
    All,
}

/// The Balances screen.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BalancesView {
    /// `"3 connections · 14 accounts"`.
    pub subtitle: String,

    /// `"Held"`. The headline is what you hold, not net worth: Plaid reports
    /// the mortgage but never the house behind it, so a net figure is
    /// structurally too red.
    pub headline_label: String,

    /// `"$86,541.74"`, or the same string masked.
    pub headline: String,

    /// Whether the figures are covered. The view draws a bar over them; the
    /// real value is still in `headline` so the layout does not shift on
    /// reveal.
    pub masked: bool,

    pub connections: Vec<ConnectionView>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ConnectionView {
    pub id: String,
    pub name: String,
    pub initials: String,

    /// `"synced 2m"`, or empty when the connection needs attention.
    pub synced: String,

    /// Whether the user has to sign in again. Carries the action, rather than
    /// just naming the problem.
    pub needs_reconnect: bool,

    /// Whether these balances are owed rather than held.
    pub is_debts: bool,

    pub accounts: Vec<AccountRow>,

    /// `"6 accounts held"` / `"6 accounts owed"`.
    pub total_label: String,
    /// `"$62,589.00"`.
    pub total: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AccountRow {
    pub id: String,
    pub name: String,

    /// `"Checking ·0000 · $100.00 available"` — the whole subtitle, assembled.
    pub subtitle: String,

    /// `"$110.00"` or `"−$56,302.06"`.
    pub amount: String,

    /// Whether this is money owed, for colour.
    pub owed: bool,

    /// `"$410 of $2,000 limit used"`, where a limit exists.
    pub utilisation: Option<String>,
}

/// The Recurring screen.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RecurringView {
    /// `"8 streams · 7 out, 1 in"`.
    pub subtitle: String,

    /// `"−$3,771.40"`, the monthly commitment.
    pub headline: String,

    /// `"+$4.22 in · net −$3,767.18"`.
    pub detail: String,

    pub masked: bool,

    /// The breakdown bar and its legend are two views of one split, so they
    /// come from one list.
    pub breakdown: Vec<BreakdownSlice>,

    pub groups: Vec<StreamGroup>,
}

/// One segment of the breakdown bar.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BreakdownSlice {
    pub label: String,
    pub token: String,
    /// The segment's share of the bar, 0..1. A ratio rather than a percentage
    /// string, because this one is geometry — the exception `docs/FFI.md`
    /// carves out for the chart.
    pub share: f64,
    /// `"$2,078.50"`.
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StreamGroup {
    /// `"Transfer out"`.
    pub name: String,
    pub token: String,
    /// `"$2,078.50"`.
    pub total: String,
    pub streams: Vec<StreamRow>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct StreamRow {
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

/// The Home screen: the highlight of each tab and nothing more.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HomeView {
    pub balances: HomeBalances,
    pub recurring: HomeRecurring,

    /// The latest few ledger rows.
    pub ledger: Vec<TxRow>,

    /// Connections needing attention. Empty when everything is syncing, and
    /// the block is not drawn at all then.
    pub attention: Vec<ConnectionView>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HomeBalances {
    /// `"$86,541.74"`.
    pub headline: String,
    pub masked: bool,
    /// `"held across 14 accounts"`.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HomeRecurring {
    /// `"−$3,771.40"`.
    pub headline: String,
    pub masked: bool,
    /// `"3 due within the week"`.
    pub detail: String,
    pub breakdown: Vec<BreakdownSlice>,
    /// Only what lands next.
    pub upcoming: Vec<StreamRow>,
}

/// Everything on screen, in one value.
///
/// One type rather than four optional ones: the view renders whichever arm it
/// is handed and holds no "which screen am I on" state of its own. That is the
/// state minimisation `docs/ARCHITECTURE.md` asks for, applied at the boundary.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum ScreenView {
    Home { view: HomeView },
    Ledger { view: LedgerView },
    Recurring { view: RecurringView },
    Balances { view: BalancesView },
}

/// The dock, and which tab is lit.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DockView {
    pub items: Vec<DockItem>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DockItem {
    pub screen: Screen,
    pub label: String,
    /// An icon name the view resolves in its own asset catalogue.
    pub icon: String,
    pub selected: bool,
}
