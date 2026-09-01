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
    /// on Recurring would be a strange thing to have to explain.
    MaskToggled,

    /// One of the two groups inside the filter panel.
    FilterGroupToggled { group: FilterGroup },

    /// A category chip. Multi-select, so this toggles rather than sets.
    CategoryToggled { id: String },

    /// Clear every category.
    CategoriesCleared,

    /// One of the four range forms.
    RangeSelected { kind: RangeSelection },

    /// A day in the recurring calendar. Tapping the day already selected
    /// clears it, so the card below can be put away the way it was opened.
    CalendarDaySelected { id: String },

    /// A swipe across the recurring calendar. `step` is months, so −1 is the
    /// month behind and 1 the month ahead.
    CalendarMonthStepped { step: i32 },

    /// The user picked a transaction out of the list.
    TxSelected { id: String },

    /// The Welcome screen's one button. There is no Sign up and no Sign in:
    /// an address that exists signs in and one that does not signs up, so
    /// there is nothing to choose and no way to choose wrong.
    EmailChosen,

    /// The address field, as it now reads.
    ///
    /// The whole value rather than one keystroke, so a paste, a backspace and
    /// a tap are the same event and the machine holds what the field holds.
    EmailTyped { text: String },

    /// Continue, under the address.
    EmailSubmitted,

    /// The code field, as it now reads. There is nothing to check before the
    /// sixth digit, so the sixth digit submits and there is no Continue.
    CodeTyped { digits: String },

    /// Send a new code.
    ResendRequested,

    /// The chevron, and the code screen's way back to a mistyped address.
    BackPressed,

    /// Connect a bank, Try again, and Add another bank.
    ///
    /// One event rather than three, because it is one thing: whichever screen
    /// the tap came from, the next thing that happens is Plaid opening.
    BankConnectRequested,

    /// Open Meridian, on the last screen of first run.
    OnboardingFinished,
}

/// Which half of the filter panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FilterGroup {
    Categories,
    Range,
}
