//! What `POST /billing/subscribe` answers with.

use serde::Deserialize;

/// A subscription that has been started and not yet paid for, and what a card
/// form needs to pay it.
#[derive(Debug, Deserialize)]
pub struct Subscribing {
    /// What the card form confirms against. Scoped to this one invoice, and
    /// kept by the runtime rather than handed to the machine.
    pub client_secret: String,

    /// What Stripe.js is opened with. Made to be in a page.
    pub publishable_key: String,
}
