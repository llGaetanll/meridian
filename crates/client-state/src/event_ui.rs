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

    /// The eye on a covered figure. One switch for the whole app.
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
    /// clears it.
    CalendarDaySelected { id: String },

    /// A swipe across the recurring calendar. `step` is months, so −1 is the
    /// month behind and 1 the month ahead.
    CalendarMonthStepped { step: i32 },

    /// The user picked a transaction out of the list.
    TxSelected { id: String },

    /// The Welcome screen's one button. There is no Sign up and no Sign in:
    /// an address that exists signs in and one that does not signs up.
    EmailChosen,

    /// The address field, as a whole value rather than one keystroke, so a
    /// paste, a backspace and a tap are the same event.
    EmailTyped { text: String },

    /// Continue, under the address.
    EmailSubmitted,

    /// The code field, as it reads. Typing alone never submits: auto-sending
    /// on the sixth digit spends a guess on a typo the user has not seen yet.
    CodeTyped { digits: String },

    /// Continue, under the six boxes. Only drawn once all six are in.
    CodeSubmitted,

    /// Send a new code.
    ResendRequested,

    /// The name field, as a whole value, for the same reason the address
    /// field is.
    NameTyped { text: String },

    /// Continue, under the name. Only drawn once there is something in it.
    NameSubmitted,

    /// The chevron, and the code screen's way back to a mistyped address.
    BackPressed,

    /// Connect a bank, Try again, and Add another bank — one event, because
    /// whichever screen the tap came from, Plaid opens next.
    BankConnectRequested,

    /// Reconnect, on a bank that has stopped answering. Opens Plaid too, in
    /// update mode against the named bank: the sign-in repairs it rather than
    /// attaching another.
    BankReconnectRequested { item_id: String },

    /// Open Meridian, on the last screen of first run.
    OnboardingFinished,

    /// Pay by card, on the paywall. Opens Stripe's own page in a window of
    /// its own; nothing is charged by the press.
    SubscribeRequested,

    /// Sign out, on the Account tab. Everything the app holds belongs to the
    /// account, so this puts the machine back to where a fresh install starts.
    SignOutRequested,

    /// Delete my account, on the Account tab. Opens the confirmation; nothing
    /// is asked of the server by the press.
    DeleteAccountRequested,

    /// Yes, on the confirmation. This is what asks the server to email the
    /// code.
    DeleteConfirmed,

    /// The way out of the delete flow at any step. Nothing already asked for
    /// is taken back — a code that was sent simply goes unused.
    DeleteCancelled,

    /// The delete code field, as it reads — a whole value for the reason
    /// [`CodeTyped`](EventUi::CodeTyped) is.
    DeleteCodeTyped { digits: String },

    /// Delete everything, under the code. Only offered once all six are in.
    DeleteCodeSubmitted,
}

/// Which half of the filter panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FilterGroup {
    Categories,
    Range,
}
