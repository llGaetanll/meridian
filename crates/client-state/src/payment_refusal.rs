//! Why a card was not taken.

/// How a payment was refused, in our words rather than a provider's.
///
/// A reason and not a sentence: the runtime says which it was, and what each
/// reads as is decided with every other string the app prints.
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum PaymentRefusal {
    /// The bank said no.
    Declined,

    /// A detail of the card cannot be right — the number, the date or the
    /// code — or one was left unfinished.
    Incorrect,

    /// The bank asked the cardholder to confirm it was them, and they did not.
    Unauthenticated,

    /// Nothing that says anything about the card: the provider could not be
    /// reached, or answered in a way the runtime could not read.
    Failed,
}
