//! What the Checkout endpoint answers with.

use serde::Deserialize;

/// One Checkout Session, cut down to the page it hosts.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeCheckoutSession {
    /// `cs_…`.
    pub id: String,

    /// The page on `checkout.stripe.com`, live for 24 hours. `None` on a
    /// session already completed or expired, which a fresh one never is.
    pub url: Option<String>,
}
