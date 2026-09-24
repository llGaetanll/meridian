//! Removing one bank, from the trash on its block to its rows going.
//!
//! A sub-flow the way [`StateDelete`](crate::StateDelete) is, and a smaller
//! one: there is no code, because what is lost is one connection's history
//! rather than the account. The way out either way — removed, or cancelled —
//! is the machine dropping it.

/// Where removing a bank has got. `None` on [`StateApp`](crate::StateApp) is
/// the ordinary state: nobody is removing anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateRemove {
    /// The "are you sure", on the bank it names.
    Asking { item_id: String },

    /// Yes pressed; the server is stopping the bank and deleting its rows.
    Sending { item_id: String },

    /// The server would not. The popup says so and the button comes back,
    /// because a bank still listed is a bank still billing.
    Refused { item_id: String },
}

impl StateRemove {
    /// Which bank, as Plaid names it.
    pub fn item_id(&self) -> &str {
        match self {
            Self::Asking { item_id } | Self::Sending { item_id } | Self::Refused { item_id } => {
                item_id
            }
        }
    }
}
