//! Which page the shell shows.

use client_state::ViewScreen;

/// The machine's four tabs, and Markets.
///
/// Markets is the web's own page and not one of the machine's screens, so
/// whether it is showing is the one choice the web holds for itself. Every
/// other page is whatever the machine last rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Ledger,
    Recurring,
    Balances,
    Account,
    Markets,
}

impl Tab {
    /// `None` on a screen the shell does not draw.
    pub fn of(screen: &ViewScreen, markets: bool) -> Option<Self> {
        let tab = match screen {
            ViewScreen::Onboarding { .. } | ViewScreen::Paywall { .. } => return None,
            _ if markets => Self::Markets,
            ViewScreen::Ledger { .. } => Self::Ledger,
            ViewScreen::Recurring { .. } => Self::Recurring,
            ViewScreen::Balances { .. } => Self::Balances,
            ViewScreen::Account { .. } => Self::Account,
        };

        Some(tab)
    }
}
