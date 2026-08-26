//! What a screen shows, already decided.
//!
//! Every type in the `view_*` modules carries finished values. A `ViewTxRow`
//! holds the date as the string the row prints and the amount as the text
//! `"−$5.40"`, not a timestamp and an `f64` for three platforms to format the
//! same way three times. `docs/FFI.md` is explicit about why: the type surface
//! gets a little larger, and the *logic* surface — the part that can disagree
//! between platforms — stays at zero.
//!
//! The rule to hold onto when adding one: if a view layer would have to write
//! an `if`, a `switch`, or a `format!` to use a field, the field is wrong.
//! Send the answer, not the ingredients.

/// `Clone` on the view types, but not on events or actions.
///
/// A view value is *data the UI owns*: a Leptos signal stores one, and SwiftUI
/// and Compose will each hold their own copy too. Events and actions are
/// messages that are passed by reference all the way down, so they need
/// nothing but `Debug`.
use crate::ViewBalances;
use crate::ViewLedger;
use crate::ViewOnboarding;
use crate::ViewRecurring;

/// Which screen the app is on.
///
/// The dock's three tabs. Search is deliberately absent — the sketch draws the
/// button but never the screen, so there is nothing to be faithful to yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum Screen {
    #[default]
    Ledger,
    Recurring,
    Balances,
}

/// Everything on screen, in one value.
///
/// One type rather than three optional ones: the view renders whichever arm it
/// is handed and holds no "which screen am I on" state of its own.
///
/// [`Onboarding`](ViewScreen::Onboarding) is not one of [`Screen`]'s tabs and
/// never will be — first run is a road with an end, not a place in the app you
/// can navigate back to.
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
    Onboarding { view: ViewOnboarding },
}
