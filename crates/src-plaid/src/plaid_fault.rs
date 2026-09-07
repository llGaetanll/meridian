//! Why a call to Plaid did not answer, as far as anything decides about it.

use crate::ITEM_LOGIN_REQUIRED;
use crate::PlaidError;

/// `/transactions/sync` against an Item whose history Plaid is still
/// assembling. Sandbox generates transactions asynchronously, so the first
/// seconds of an Item's life answer this.
pub const PRODUCT_NOT_READY: &str = "PRODUCT_NOT_READY";

/// Ours rather than Plaid's: a call that never got far enough for there to be
/// a code to read — no network, a body that would not parse, a 429.
pub const REQUEST_FAILED: &str = "REQUEST_FAILED";

/// Ours as well: a bank we hold no access token for, so there was nothing to
/// sign a call with.
pub const NO_ACCESS_TOKEN: &str = "NO_ACCESS_TOKEN";

/// What went wrong with one call to Plaid.
///
/// A classification rather than a copy of Plaid's code list: a variant exists
/// where something decides differently because of it, and everything else is
/// [`Failed`](PlaidFault::Failed) under whatever code came back. A value
/// rather than a string, so a rule about one of them is a `match` and not a
/// comparison against a spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaidFault {
    /// The bank wants the person to sign in again. No amount of asking fixes
    /// it, which is what makes it the one fault worth writing down.
    LoginRequired,

    /// The Item is fine and what was asked for does not exist yet. The one
    /// fault where asking again is the answer.
    NotReady,

    /// We hold no access token for this bank, so Plaid was never asked.
    ///
    /// Its own variant because it is not Plaid's answer at all: it means what
    /// the runtime holds and what the store holds have drifted, and no amount
    /// of asking Plaid settles that.
    NoToken,

    /// Anything else, under the code it came with.
    Failed(String),
}

impl PlaidFault {
    /// The fault one of Plaid's error codes names.
    pub fn from_code(code: &str) -> Self {
        match code {
            ITEM_LOGIN_REQUIRED => PlaidFault::LoginRequired,
            PRODUCT_NOT_READY => PlaidFault::NotReady,
            other => PlaidFault::Failed(other.to_string()),
        }
    }

    /// What went wrong with a call, whether or not Plaid got to say.
    pub fn of(error: &PlaidError) -> Self {
        match error {
            PlaidError::Plaid { error_code, .. } => Self::from_code(error_code),
            _ => PlaidFault::Failed(REQUEST_FAILED.to_string()),
        }
    }

    /// The code this is, for a log line and for the Item's row.
    pub fn as_str(&self) -> &str {
        match self {
            PlaidFault::LoginRequired => ITEM_LOGIN_REQUIRED,
            PlaidFault::NotReady => PRODUCT_NOT_READY,
            PlaidFault::NoToken => NO_ACCESS_TOKEN,
            PlaidFault::Failed(code) => code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_with_rules_about_them_are_their_own_faults() {
        assert_eq!(
            PlaidFault::from_code(ITEM_LOGIN_REQUIRED),
            PlaidFault::LoginRequired,
        );
        assert_eq!(
            PlaidFault::from_code(PRODUCT_NOT_READY),
            PlaidFault::NotReady,
        );
    }

    /// A code nobody has a rule about survives whole: it is what the Item's
    /// row is written with and what the log line says.
    #[test]
    fn anything_else_keeps_its_code() {
        let fault = PlaidFault::from_code("INTERNAL_SERVER_ERROR");

        assert_eq!(fault, PlaidFault::Failed("INTERNAL_SERVER_ERROR".into()));
        assert_eq!(fault.as_str(), "INTERNAL_SERVER_ERROR");
    }

    /// A call that never reached Plaid has no code of Plaid's to carry, and
    /// it still has to be one of these.
    #[test]
    fn a_call_that_never_got_an_answer_is_a_failure_of_ours() {
        let fault = PlaidFault::of(&PlaidError::TooManyRequests);

        assert_eq!(fault.as_str(), REQUEST_FAILED);
    }

    /// The two faults that are ours have to stay apart: one is Plaid refusing
    /// and the other is Plaid never being asked, and only the second says the
    /// tokens we hold and the rows we hold disagree.
    #[test]
    fn a_bank_with_no_token_is_not_a_failed_call() {
        assert_ne!(PlaidFault::NoToken.as_str(), REQUEST_FAILED);
        assert_eq!(PlaidFault::NoToken.as_str(), NO_ACCESS_TOKEN);
    }
}
