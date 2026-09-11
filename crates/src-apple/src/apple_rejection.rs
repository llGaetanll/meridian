//! Why something Apple signed was not believed.

/// The reasons a signed payload is refused, one per check it can fail.
///
/// Separate variants rather than a bool, because they read differently in a
/// log. An [`Algorithm`](AppleRejection::Algorithm) is somebody probing; an
/// [`Expired`](AppleRejection::Expired) on a chain that otherwise verifies is
/// far more likely to be a clock than an attack.
///
/// None of them reaches the caller. Apple gets one status for every way of
/// being wrong, because the difference is only useful to whoever is guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppleRejection {
    /// Not something we can take apart. Says which part gave up.
    Malformed(&'static str),

    /// The header names an algorithm that is not `ES256`.
    ///
    /// Checked before any certificate is parsed, and that order is the point:
    /// `alg: none` and a symmetric `alg` are the two classic ways a verifier
    /// is talked out of verifying.
    Algorithm(String),

    /// The header carries no `x5c`, or too few certificates to reach a root.
    /// There is nothing to check the signature against.
    Unchained,

    /// A certificate in the chain cannot be read. Says which part gave up.
    Certificate(&'static str),

    /// A certificate that signed another one is not a certificate authority.
    ///
    /// The check that stops anybody holding an ordinary Apple-issued
    /// certificate — a developer's, a push certificate — from signing a leaf
    /// of their own and having the arithmetic come out right.
    NotACa,

    /// The leaf is not marked as an App Store signing certificate.
    Purpose,

    /// A certificate in the chain is outside its validity window, in either
    /// direction.
    Expired,

    /// A certificate's signature does not match the one above it, so the chain
    /// does not reach the root we trust.
    Untrusted,

    /// The payload's own signature does not match the leaf that carried it.
    Signature,
}

impl std::fmt::Display for AppleRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AppleRejection::Malformed(what) => write!(f, "malformed JWS: {what}"),
            AppleRejection::Algorithm(alg) => write!(f, "algorithm is `{alg}`, not ES256"),
            AppleRejection::Unchained => write!(f, "the header carries no certificate chain"),
            AppleRejection::Certificate(what) => write!(f, "unreadable certificate: {what}"),
            AppleRejection::NotACa => write!(f, "a signing certificate is not an authority"),
            AppleRejection::Purpose => write!(f, "the leaf does not sign for the App Store"),
            AppleRejection::Expired => write!(f, "a certificate is outside its validity window"),
            AppleRejection::Untrusted => write!(f, "the chain does not reach Apple's root"),
            AppleRejection::Signature => write!(f, "the signature does not match"),
        }
    }
}

impl std::error::Error for AppleRejection {}
