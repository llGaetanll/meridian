//! The user did something.

use crate::RangeSelection;
use crate::Screen;

#[derive(Debug, uniffi::Enum)]
pub enum EventUi {
    /// The app came up.
    Started,

    /// A tab in the dock.
    ScreenOpened { screen: Screen },

    /// Pull to refresh.
    RefreshRequested,

    /// The eye on a covered figure.
    ///
    /// One switch for the whole app: revealing on Balances and staying covered
    /// on Home would be a strange thing to have to explain.
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
