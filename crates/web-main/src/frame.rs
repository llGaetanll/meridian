//! What the window holds.

use client_state::ViewScreen;

/// Which screen is showing, without what is on it.
///
/// Switching on this rather than on the screen is what mounts a page once per
/// visit: a render of the same screen changes the screen and not its frame,
/// so it reaches the page as a new value on its signal instead of a new page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// First run. No shell: it is a road with an end, not a place in the app.
    Onboarding,

    /// No shell either. Behind it there is no app to navigate.
    Paywall,

    /// One of the machine's tabs, inside the sidebar.
    Shell,
}

impl Frame {
    pub fn of(screen: &ViewScreen) -> Self {
        match screen {
            ViewScreen::Onboarding { .. } => Self::Onboarding,
            ViewScreen::Paywall { .. } => Self::Paywall,
            ViewScreen::Ledger { .. }
            | ViewScreen::Recurring { .. }
            | ViewScreen::Connections { .. }
            | ViewScreen::Account { .. } => Self::Shell,
        }
    }
}
