//! A request to Plaid, built but not yet sent.

use serde_json::Value;

/// One call to Plaid, built and ready to go on the wire.
///
/// The point of the type is that building is not sending. Turning a
/// [`PlaidRequest`](crate::PlaidRequest) into this is pure — no socket, no
/// clock, nothing that can fail at a distance — so every way of being wrong
/// about a request that does not involve Plaid answering is reachable from a
/// test with no network and no mock in it.
///
/// Unlike the [`PlaidRequest`](crate::PlaidRequest) it came from, it carries
/// the credentials. That is the whole difference between the two, and it is
/// deliberate: this is what goes out, so this is what gets logged. A response
/// tells us what Plaid thought we asked for and never what we actually sent.
///
/// There is no method field. Plaid is POST throughout, which
/// [`PlaidRequest`](crate::PlaidRequest) says and no endpoint has yet
/// contradicted.
#[derive(Debug, Clone)]
pub struct PlaidHttpRequest {
    /// Absolute, already resolved against the client's base — so it names the
    /// environment, which is the first thing a request line has to say.
    pub url: reqwest::Url,

    /// The body as it will be serialised, `client_id` and `secret` included.
    pub body: Value,
}
