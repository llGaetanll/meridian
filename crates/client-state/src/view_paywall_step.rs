//! Where paying is, as the paywall draws it.

/// The part of the paywall that moves as somebody pays: one button, and what
/// it is waiting on.
///
/// An enum rather than flags, for the reason [`ViewOnboarding`](crate::ViewOnboarding)
/// is one: which event the button sends is which arm this is, so the view
/// holds no "which step am I on" of its own.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewPaywallStep {
    /// Pay by card, and the money is taken on Stripe's own page. The button
    /// sends [`SubscribeRequested`](crate::EventUi::SubscribeRequested).
    Offer {
        /// `"Pay by card"`, or what it is waiting on once pressed.
        action: String,
        /// Whether Stripe's page is on its way or up. The button is locked
        /// while it is: the money is being taken there or not at all.
        opening: bool,
    },

    /// The sheet took the money, and the server has not heard yet. While
    /// `checking` it is being asked on its own; after that the button sends
    /// [`RefreshRequested`](crate::EventUi::RefreshRequested).
    Confirming { action: String, checking: bool },
}
