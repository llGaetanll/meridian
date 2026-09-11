//! What confirming a card came to.

/// How a payment went, in our words rather than Stripe's.
///
/// `Taken` is Stripe's word, not the server's: nobody is entitled until
/// Stripe's webhook reaches the server, which is a moment later.
#[derive(Debug, Clone, Copy)]
pub enum Payment {
    /// The card paid.
    Taken,

    /// The bank said no.
    Declined,

    /// A detail of the card cannot be right — the number, the date or the
    /// code — or one was left unfinished.
    Incorrect,

    /// The bank asked the cardholder to confirm it was them, and they did not.
    Unauthenticated,

    /// Nothing that says anything about the card: Stripe unreachable, a form
    /// that never opened, or an answer this build cannot read.
    Failed,
}
