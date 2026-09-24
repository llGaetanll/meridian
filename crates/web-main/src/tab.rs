//! Which page the shell shows.

use client_state::ViewScreen;
use leptos::prelude::*;

/// The machine's four tabs, and Markets.
///
/// Markets is the web's own page and not one of the machine's screens, so
/// whether it is showing is the one choice the web holds for itself. Every
/// other page is whatever the machine last rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Ledger,
    Recurring,
    Connections,
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
            ViewScreen::Connections { .. } => Self::Connections,
            ViewScreen::Account { .. } => Self::Account,
        };

        Some(tab)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ledger => "Ledger",
            Self::Recurring => "Recurring",
            Self::Connections => "Connections",
            Self::Account => "Account",
            Self::Markets => "Markets",
        }
    }
}

/// The app's name, which the title falls back to on a page that is on no tab.
const NAME: &str = "Meridian";

/// Keep the browser's tab named after the app's tab, and after nothing else.
///
/// No suffix and no separator. Someone reading a strip of windows to tell
/// them apart learns nothing from four copies of "Meridian".
pub fn follow_title(tab: Signal<Option<Tab>>) {
    Effect::new(move |_| set_title(tab.get().map_or(NAME, Tab::label)));
}

/// For onboarding and the paywall, which are on no tab and outside the shell
/// that follows one.
pub fn name_title() {
    set_title(NAME);
}

fn set_title(title: &str) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };

    document.set_title(title);
}
