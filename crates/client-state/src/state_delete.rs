//! Deleting the account, from the button on the Account tab to the rows
//! going.
//!
//! A sub-flow the way [`StateOnboarding`](crate::StateOnboarding) is: the
//! machine holds where it is, the view draws it, and the way out either way —
//! deleted, or cancelled — is the machine dropping it.

/// Where deleting the account has got. `None` on [`StateApp`](crate::StateApp)
/// is the ordinary state: nobody is leaving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateDelete {
    /// The "are you sure" popup.
    Confirming,

    /// Yes pressed; the server is being asked to email the code.
    SendingCode,

    /// The code screen: the digits typed so far, and whether the last try
    /// was refused.
    Code { digits: String, wrong: bool },

    /// The code is submitted, and the account is being deleted.
    Deleting,
}
