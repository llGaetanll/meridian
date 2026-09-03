//! Why a webhook was not believed.

/// The reasons a webhook is refused, one per check it can fail.
///
/// Separate variants rather than a bool, and separate from
/// [`PlaidError`](crate::PlaidError) which is about calls *we* make. What
/// refused a webhook is the whole content of the log line about it: an
/// [`Algorithm`](PlaidRejection::Algorithm) is somebody probing, a
/// [`Body`](PlaidRejection::Body) is far more likely to be us re-serialising a
/// body we should have left alone.
///
/// None of them reaches the caller. Plaid gets one status for every way of
/// being wrong, because the difference is only useful to whoever is guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaidRejection {
    /// No `Plaid-Verification` header at all, so there is nothing to check.
    Unsigned,

    /// Not something we can take apart — the token, or, once the token has
    /// vouched for it, the body. Says which part gave up.
    Malformed(&'static str),

    /// The JWT names an algorithm that is not `ES256`.
    ///
    /// Checked before anything else is done with the token, and that order is
    /// the point: `alg: none` and a symmetric `alg` are the two classic ways a
    /// verifier is talked out of verifying, and neither should get as far as a
    /// key lookup.
    Algorithm(String),

    /// The JWT header carries no `kid`, so there is no key to ask for.
    Unkeyed,

    /// The key fetched for this `kid` cannot check this signature — a
    /// different curve, a different algorithm, or an id that came back naming
    /// something else.
    Unusable,

    /// The signature does not match the key.
    Signature,

    /// `iat` is outside the replay window. Carries how far, in seconds, so a
    /// clock that has drifted looks different in the log from a token being
    /// replayed hours later.
    Stale { seconds: i64 },

    /// The body does not hash to what the JWT says it does. Most often *our*
    /// mistake rather than an attack: Plaid hashes the bytes it sent, so a
    /// body that was parsed and printed again never matches.
    Body,
}

impl std::fmt::Display for PlaidRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PlaidRejection::Unsigned => write!(f, "no Plaid-Verification header"),
            PlaidRejection::Malformed(what) => write!(f, "malformed JWT: {what}"),
            PlaidRejection::Algorithm(alg) => write!(f, "algorithm is `{alg}`, not ES256"),
            PlaidRejection::Unkeyed => write!(f, "the JWT header names no key"),
            PlaidRejection::Unusable => write!(f, "the key cannot check an ES256 signature"),
            PlaidRejection::Signature => write!(f, "the signature does not match"),
            PlaidRejection::Stale { seconds } => write!(f, "issued {seconds}s away from now"),
            PlaidRejection::Body => write!(f, "the body does not match its hash"),
        }
    }
}

impl std::error::Error for PlaidRejection {}
