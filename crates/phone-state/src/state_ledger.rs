//! The Ledger screen's own state.
//!
//! A sub-machine rather than fields on [`StateApp`](crate::StateApp): the
//! filter is the one piece of state that belongs to a single screen, and
//! keeping it here means the app's machine never has to know what a category
//! chip is.
//!
//! Note what is *not* here: the entries. They are the ledger, and the ledger is
//! the app's — Home previews it, Recurring is derived from the same feed, and a
//! copy per screen would be a copy that can disagree. A screen's machine owns
//! the screen's *choices*; the data stays in one place.
//!
//! It handles no events of its own. [`StateApp`](crate::StateApp) routes the
//! ones that are the Ledger's business here, which keeps a single
//! `handle_events` as the only way anything changes.

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
    open: bool,
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
            open: false,
            // Both groups start open, because a panel that opens showing two
            // collapsed headings has told the user nothing.
            categories_open: true,
            range_open: true,
            categories: Vec::new(),
            range: RangeSelection::All,
        }
    }
}

impl StateLedger {
    pub(crate) fn toggle_panel(&mut self) {
        self.open = !self.open;
    }

    pub(crate) fn toggle_group(&mut self, group: FilterGroup) {
        match group {
            FilterGroup::Categories => self.categories_open = !self.categories_open,
            FilterGroup::Range => self.range_open = !self.range_open,
        }
    }

    /// Add or remove a category. Multi-select, so this toggles rather than sets.
    ///
    /// An unknown category is ignored rather than filed under `Unknown`: an id
    /// the view invented should narrow nothing, not silently filter to a
    /// category no transaction is in.
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

    pub(crate) fn is_open(&self) -> bool {
        self.open
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

                // The end is a day, and a day includes its own transactions, so
                // the window runs to the end of it rather than to midnight at
                // its start.
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
    /// Separate because the category chips' counts need it: a count computed
    /// through the current category selection would read zero for everything
    /// unselected, when what a chip has to say is what selecting *it* would
    /// give. The two filters are independent, so they are asked independently.
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

    /// Which tokens the range sentence draws.
    ///
    /// They keep their last value even when another form is live, so switching
    /// back does not reset the sentence.
    pub(crate) fn range_units(&self) -> (RangeUnit, u32, RangeUnit) {
        match self.range {
            RangeSelection::This { unit } => (unit, 3, RangeUnit::Month),
            RangeSelection::Last { count, unit } => (RangeUnit::Month, count, unit),
            _ => (RangeUnit::Month, 3, RangeUnit::Month),
        }
    }
}
