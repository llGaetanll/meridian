//! What a Link session came to.

/// Three answers, not two. Closing Plaid's page says nothing on its own —
/// Plaid is explicit that reaching the end of it is not a successful
/// connection — so the server is the only party that sees both endings, and it
/// may not have seen the second one yet.
///
/// None of them names the bank. Nothing knows what one is called until the
/// balances call inside its first sync answers.
#[derive(Debug, Clone, Copy)]
pub enum LinkOutcome {
    /// At least one bank is attached.
    Linked,

    /// The session ended without one. Plaid said so.
    Abandoned,

    /// Plaid has not said. The bank may still be landing.
    Pending,
}
