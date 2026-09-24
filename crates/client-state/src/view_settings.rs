//! The settings window: the one thing the app draws that is not a screen.
//!
//! Its own value rather than a field on a screen, for the reason
//! [`ViewWho`](crate::ViewWho) is one: the window opens over whatever page is
//! up and leaves it where it was, so what it draws cannot belong to any of
//! them.
//!
//! Small on purpose. A setting that configures one screen belongs on that
//! screen; what is here is what belongs to the account itself — the name the
//! app calls you by, and the door with nothing behind it.

use crate::ViewAccountDelete;
use crate::ViewAccountName;

/// The settings window, whether or not anything has opened it. Whether it is
/// open is the platform's: no event tells the machine, because nothing the
/// machine decides turns on it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewSettings {
    /// `"Settings"`, over the window.
    pub title: String,

    /// The one thing in the rail, and the heading of the page it stands on.
    pub account_label: String,

    /// What the close button is called, for a button drawn as a glyph.
    pub close_label: String,

    /// The name the app calls them by, and the field it is changed in. The
    /// same value the Account screen carries.
    pub name: Option<ViewAccountName>,

    /// `"Delete my account"`.
    pub delete_label: String,

    /// The delete confirmation, while somebody is leaving.
    pub delete: Option<ViewAccountDelete>,
}
