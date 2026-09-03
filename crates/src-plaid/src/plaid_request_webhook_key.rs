use serde_json::Value;

use crate::PlaidRequest;

/// `/webhook_verification_key/get` — the public key one webhook was signed with.
///
/// Plaid signs every webhook with a rotating key and names the one it used in
/// the JWT's `kid`. There is no key list to fetch ahead of time: the key is
/// asked for by id, after a webhook has arrived carrying that id.
///
/// **The caller caches the answer, and caches the misses too.** This request is
/// caused by an anonymous POST, so a `kid` nobody has ever seen would otherwise
/// be a way to make this server call Plaid as often as somebody likes. See
/// [`PlaidWebhookJwt`](crate::PlaidWebhookJwt), whose `alg` check is what a
/// caller does *before* reaching for this.
pub struct WebhookKeyRequest {
    /// The `kid` off the JWT header, which is the only place it comes from.
    pub key_id: String,
}

impl WebhookKeyRequest {
    pub fn new(key_id: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into(),
        }
    }
}

impl PlaidRequest for WebhookKeyRequest {
    fn path(&self) -> String {
        String::from("webhook_verification_key/get")
    }

    fn body(&self) -> Value {
        serde_json::json!({ "key_id": self.key_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_asked_for_by_id() {
        let req = WebhookKeyRequest::new("6c5516e1-92dc-479e-a8ff-5a51992e0001");

        assert_eq!(req.path(), "webhook_verification_key/get");
        assert_eq!(req.body()["key_id"], "6c5516e1-92dc-479e-a8ff-5a51992e0001");
    }
}
