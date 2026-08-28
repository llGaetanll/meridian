//! What `/sandbox/public_token/create` answers with.

use serde::Deserialize;

/// Indistinguishable from what a finished Link session produces, which is the
/// point: the token goes to `/item/public_token/exchange` the same way, and
/// nothing downstream knows a browser was never involved.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SandboxPublicTokenResponse {
    pub public_token: String,
}
