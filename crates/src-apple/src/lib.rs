//! Apple, which is how a subscription is paid for on the phone.
//!
//! What arrives from Apple is a JWS: a header naming the certificate chain
//! that signed it, a payload, and an ES256 signature over both. The same
//! format arrives twice — once from the app, carrying the transaction somebody
//! has just bought, and once from Apple's own servers when a renewal, a
//! cancellation or a refund happens later — so one verifier serves both.
//!
//! There is no shared secret here, as there is with Stripe. Apple signs with a
//! certificate, and the only thing this server knows in advance is
//! [`APPLE_ROOT_CA_G3`]. Everything else is shown to it, and has to be proved.
//!
//! Pure, and unusually so for an `src-*` crate: nothing is asked of Apple in
//! order to believe what Apple sent.

mod apple_chain;
mod apple_jws;
mod apple_key;
mod apple_rejection;
mod apple_root;

pub use apple_jws::verify;
pub use apple_rejection::AppleRejection;
pub use apple_root::APPLE_ROOT_CA_G3;

pub(crate) use apple_key::AppleKey;
