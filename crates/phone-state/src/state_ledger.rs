//! The Ledger screen's own state.
//!
//! A sub-machine rather than fields on [`StateApp`](crate::StateApp): the
//! filter is the one piece of state that belongs to a single screen. The
//! entries are not here — they stay on the app, because a copy per screen is a
//! copy that can disagree — and this machine handles no events of its own.

use api_common::Timestamp;
use api_ledger::Category;
use api_ledger::Entry;

use crate::FilterGroup;
use crate::RangeSelection;
use crate::RangeUnit;
use crate::fmt;

/// What the Ledger is filtered by, and what its panel is showing.
#[derive(Debug)]
pub struct StateLedger {
    categories_open: bool,
    range_open: bool,

    /// Empty means everything, rather than nothing. That is the only reading
    /// that makes an unused filter invisible.
    categories: Vec<Category>,

    range: RangeSelection,
}

impl Default for StateLedger {
    fn default() -> Self {
        Self {
            // Both groups start shut: the panel is always drawn, and two open
            // groups are a screen of filter in front of the list they filter.
            categories_open: false,
            range_open: false,
            categories: Vec::new(),
            range: RangeSelection::All,
        }
    }
}

impl StateLedger {
    pub(crate) fn toggle_group(&mut self, group: FilterGroup) {
        match group {
            FilterGroup::Categories => self.categories_open = !self.categories_open,
            FilterGroup::Range => self.range_open = !self.range_open,
        }
    }

    /// Add or remove a category. Multi-select, so this toggles rather than sets.
    ///
    /// `Unknown` is ignored: an id the view invented should narrow nothing,
    /// not filter to a category no transaction is in.
    pub(crate) fn toggle_category(&mut self, category: Category) {
        if category == Category::Unknown {
            return;
        }

        match self.categories.iter().position(|c| *c == category) {
            Some(at) => {
                self.categories.remove(at);
            }
            None => self.categories.push(category),
        }

        debug_assert!(
            self.categories.len() <= Category::ALL.len(),
            "a category was selected twice",
        );
    }

    pub(crate) fn clear_categories(&mut self) {
        self.categories.clear();
    }

    pub(crate) fn set_range(&mut self, range: RangeSelection) {
        self.range = range;
    }

    pub(crate) fn categories_open(&self) -> bool {
        self.categories_open
    }

    pub(crate) fn range_open(&self) -> bool {
        self.range_open
    }

    pub(crate) fn categories(&self) -> &[Category] {
        &self.categories
    }

    pub(crate) fn range(&self) -> RangeSelection {
        self.range
    }

    /// Whether this filter is actually narrowing anything.
    pub(crate) fn is_active(&self) -> bool {
        !self.categories.is_empty() || self.range != RangeSelection::All
    }

    /// The window the range covers. `None` at either end means unbounded.
    pub(crate) fn window(&self, now: Timestamp) -> (Option<Timestamp>, Option<Timestamp>) {
        match self.range {
            RangeSelection::All => (None, None),

            // Exact: "this month" is from the first of the month.
            RangeSelection::This { unit } => (Some(fmt::start_of(unit, now)), None),

            // Approximate, and meant to be: "last 3 months" is about three
            // months back, not a calendar boundary.
            RangeSelection::Last { count, unit } => {
                let span = unit.seconds().saturating_mul(count.max(1) as u64);
                (Some(now.saturating_sub_secs(span)), None)
            }

            RangeSelection::Between { .. } => {
                let Some((start, end)) = self.range.window_bounds() else {
                    unreachable!("the arm matched Between")
                };

                // The end is a day, and a day includes its own transactions.
                (Some(start), Some(end.saturating_add_secs(86_400)))
            }
        }
    }

    /// Whether an entry survives this filter.
    pub(crate) fn matches(&self, entry: &Entry, now: Timestamp) -> bool {
        self.matches_range(entry, now) && self.matches_category(entry)
    }

    fn matches_category(&self, entry: &Entry) -> bool {
        self.categories.is_empty() || self.categories.contains(&entry.category)
    }

    /// The range half on its own.
    ///
    /// Separate because the category chips' counts need it: a count taken
    /// through the current selection would read zero for everything
    /// unselected, when a chip has to say what selecting *it* would give.
    pub(crate) fn matches_range(&self, entry: &Entry, now: Timestamp) -> bool {
        let (start, end) = self.window(now);
        let ts = entry.ts();

        if let Some(start) = start
            && ts < start
        {
            return false;
        }

        if let Some(end) = end
            && ts >= end
        {
            return false;
        }

        true
    }

    /// Which tokens the range sentence draws. They keep their last value even
    /// when another form is live, so switching back does not reset it.
    pub(crate) fn range_units(&self) -> (RangeUnit, u32, RangeUnit) {
        match self.range {
            RangeSelection::This { unit } => (unit, 3, RangeUnit::Month),
            RangeSelection::Last { count, unit } => (RangeUnit::Month, count, unit),
            _ => (RangeUnit::Month, 3, RangeUnit::Month),
        }
    }
}
