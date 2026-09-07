//! What a Plaid Link session came to.

use crate::ActionRouter;

/// The two endings a bank sign-in has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOutcome {
    Linked,

    /// The sheet was closed, or the exchange failed. The common ending.
    Nothing,
}

impl LinkOutcome {
    pub fn answer(self) -> ActionRouter {
        match self {
            LinkOutcome::Linked => ActionRouter::Linked { count: 1 },
            LinkOutcome::Nothing => ActionRouter::LinkAbandoned,
        }
    }
}
