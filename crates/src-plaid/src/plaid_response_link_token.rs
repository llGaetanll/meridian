//! What `/link/token/create` answers with.

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LinkTokenResponse {
    /// Identifies this Link session. It is also what
    /// [`LinkSessionRequest`](crate::LinkSessionRequest) is asked about
    /// afterwards, so it is worth holding onto until the session is finished
    /// with.
    pub link_token: String,

    /// When the token stops working, ISO 8601. Short — tens of minutes — so a
    /// link URL is made when someone taps Connect and not before.
    #[serde(default)]
    pub expiration: Option<String>,

    /// The page to open. Present only because the request asked for Hosted
    /// Link; the SDK flow answers without one.
    #[serde(default)]
    pub hosted_link_url: Option<String>,
}
