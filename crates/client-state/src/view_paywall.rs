//! What somebody sees when the session is good and has not been paid for.
//!
//! **Not an error screen.** Nothing has gone wrong: the account exists, the
//! token works, and the server is answering. It is the same screen the web
//! draws for the same refusal.

use crate::ViewIconRow;
use crate::ViewRun;

/// The paywall, in the words it is read in.
///
/// No Subscribe: nothing on this phone can take money yet, and a button that
/// looked live and did nothing would be worse than one that is not there.
/// Signing out is the only thing that can be done from here, so it is the only
/// thing offered.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewPaywall {
    pub title: String,

    pub body: Vec<ViewRun>,

    /// What it costs, and when the last period ran out where there was one.
    pub rows: Vec<ViewIconRow>,

    /// `"Sign out"`.
    pub sign_out: String,
}
