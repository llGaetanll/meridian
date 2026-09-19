//! What somebody sees when the session is good and has not been paid for.
//!
//! **Not an error screen.** Nothing has gone wrong: the account exists, the
//! token works, and the server is answering. It is the same screen the web
//! draws for the same refusal.

use crate::ViewIconRow;
use crate::ViewPaywallStep;
use crate::ViewRun;

/// The paywall, in the words it is read in.
///
/// The phone draws no Subscribe: it can take money only through the App
/// Store, which it does not do yet, and a button that looked live and did
/// nothing would be worse than one that is not there. So it draws everything
/// here but [`step`](Self::step), and the web and the desktop app, which take a
/// card, draw that too.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewPaywall {
    pub title: String,

    pub body: Vec<ViewRun>,

    /// What it costs, and when the last period ran out where there was one.
    pub rows: Vec<ViewIconRow>,

    /// How far paying has got: the button, and the card form where there is
    /// one.
    pub step: ViewPaywallStep,

    /// `Some("Sign out")` — absent behind Stripe's window, where the only
    /// way anywhere is the window itself.
    pub sign_out: Option<String>,
}
