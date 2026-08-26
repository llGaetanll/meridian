use serde_json::Value;

use crate::PlaidRequest;

/// `/link/token/get` — how a finished Hosted Link session is collected.
///
/// The completion redirect carries no token; it only closes the web session.
/// The `public_token` reaches us on one of two channels, and this is the one
/// that needs no public URL: ask what happened to the link token. Plaid holds
/// the answer for six hours after the session finishes, so a poll that starts
/// when the sheet closes cannot miss it.
///
/// The other channel is the `SESSION_FINISHED` webhook, which is prompter and
/// is what a deployed server should prefer. This works from a laptop, which
/// the webhook does not.
pub struct LinkSessionRequest {
    pub link_token: String,
}

impl LinkSessionRequest {
    pub fn new(link_token: impl Into<String>) -> Self {
        Self {
            link_token: link_token.into(),
        }
    }
}

impl PlaidRequest for LinkSessionRequest {
    fn path(&self) -> String {
        String::from("link/token/get")
    }

    fn body(&self) -> Value {
        serde_json::json!({ "link_token": self.link_token })
    }
}
