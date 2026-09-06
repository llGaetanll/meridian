//! What a screen shows, already decided.
//!
//! Every type in the `view_*` modules carries finished values: the amount is
//! the text `"−$5.40"`, not an `f64` for three platforms to format three ways.
//! If a view layer would have to write an `if`, a `switch` or a `format!` to
//! use a field, the field is wrong (`docs/FFI.md`). They are `Clone` where
//! events and actions are not, because a view value is data the UI owns.

use crate::ViewAccount;
use crate::ViewBalances;
use crate::ViewLedger;
use crate::ViewOnboarding;
use crate::ViewRecurring;

/// Which screen the app is on — the dock's four tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum Screen {
    #[default]
    Ledger,
    Recurring,
    Balances,
    Account,
}

/// Everything on screen, in one value, so the view holds no "which screen am
/// I on" state of its own. [`Onboarding`](ViewScreen::Onboarding) is not one
/// of [`Screen`]'s tabs — first run has an end.
#[derive(Debug, Clone, uniffi::Enum)]
#[expect(
    clippy::large_enum_variant,
    reason = "one of these is built per render and handed straight across the \
              FFI, so the few hundred bytes between the largest variant and \
              the smallest are paid once per event and never moved again. \
              Boxing the payload would not cross UniFFI."
)]
pub enum ViewScreen {
    Ledger { view: ViewLedger },
    Recurring { view: ViewRecurring },
    Balances { view: ViewBalances },
    Account { view: ViewAccount },
    Onboarding { view: ViewOnboarding },
}
