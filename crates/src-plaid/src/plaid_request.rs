use serde_json::Value;

/// One call to Plaid.
///
/// Plaid is POST-and-JSON throughout: every endpoint takes a body, and the
/// credentials go in that body rather than in a header. Implementations return
/// the endpoint-specific half and [`PlaidClient`](crate::PlaidClient) adds
/// `client_id` and `secret` before sending, so a request type never holds a
/// secret and cannot leak one by being logged or cloned.
pub trait PlaidRequest {
    /// Path relative to the API host, e.g. `transactions/sync`.
    ///
    /// Relative for the same reason as the price sources: the base is what
    /// picks Sandbox or Production, and the same request works against either.
    fn path(&self) -> String;

    /// The request body, minus credentials. Must be a JSON object — the client
    /// inserts into it.
    fn body(&self) -> Value;
}
