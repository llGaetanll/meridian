//! Where paying is, as the paywall draws it.

/// The part of the paywall that moves as somebody pays: the button, and the
/// card form where there is one.
///
/// An enum rather than flags, for the reason [`ViewOnboarding`](crate::ViewOnboarding)
/// is one: which event the button sends is which arm this is, so the view
/// holds no "which step am I on" of its own.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewPaywallStep {
    /// Subscribe, and nothing started. The button sends
    /// [`SubscribeRequested`](crate::EventUi::SubscribeRequested).
    Offer {
        /// `"Subscribe"`, or what it is waiting on once pressed.
        action: String,
        /// Whether the button is waiting on the subscription being started.
        opening: bool,
    },

    /// The card form, and Pay under it. The button sends
    /// [`PaymentRequested`](crate::EventUi::PaymentRequested).
    Card {
        /// `"Pay $5.99"`, or what it is waiting on once pressed.
        action: String,
        /// Whether the card is with the provider. The form is locked while it
        /// is.
        paying: bool,
        /// Why the last card was refused, under the form. Absent until one
        /// has been.
        fault: Option<String>,
    },

    /// The card was taken, and the server has not heard yet. While `checking`
    /// it is being asked on its own; after that the button sends
    /// [`RefreshRequested`](crate::EventUi::RefreshRequested).
    Confirming { action: String, checking: bool },
}
