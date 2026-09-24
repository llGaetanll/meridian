//! What a screen shows, already decided.
//!
//! Every type in the `view_*` modules carries finished values: the amount is
//! the text `"−$5.40"`, not an `f64` for three platforms to format three ways.
//! If a view layer would have to write an `if`, a `switch` or a `format!` to
//! use a field, the field is wrong (`docs/FFI.md`). They are `Clone` where
//! events and actions are not, because a view value is data the UI owns.

use crate::ViewAccount;
use crate::ViewConnections;
use crate::ViewLedger;
use crate::ViewOnboarding;
use crate::ViewPaywall;
use crate::ViewRecurring;

/// Which screen the app is on — the dock's four tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum Screen {
    #[default]
    Ledger,
    Recurring,
    Connections,
    Account,
}

/// Everything on screen, in one value, so the view holds no "which screen am
/// I on" state of its own. [`Onboarding`](ViewScreen::Onboarding) is not one
/// of [`Screen`]'s tabs — first run has an end — and neither is
/// [`Paywall`](ViewScreen::Paywall), which stands in front of both.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewScreen {
    Ledger { view: ViewLedger },
    Recurring { view: ViewRecurring },
    Connections { view: ViewConnections },
    Account { view: ViewAccount },
    Onboarding { view: ViewOnboarding },
    Paywall { view: ViewPaywall },
}
