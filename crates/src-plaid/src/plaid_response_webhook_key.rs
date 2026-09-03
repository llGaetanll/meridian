//! What `/webhook_verification_key/get` answers with.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookKeyResponse {
    pub key: PlaidJwk,
}

/// One signing key, as a JWK.
///
/// A P-256 public key in the two halves of its coordinate, base64url with no
/// padding — which is the JWK spelling of a point and not anything Plaid
/// invented. Only what a signature check needs is parsed; `created_at` and the
/// rest of the record say nothing a verifier acts on.
///
/// `expired_at` is the exception, and it is deliberately *not* enforced here.
/// Plaid retires a key by giving later webhooks a new `kid`, so a webhook
/// signed before the retirement is still genuine — refusing it would reject
/// real traffic to guard against a replay the five-minute `iat` window already
/// covers.
#[derive(Debug, Clone, Deserialize)]
pub struct PlaidJwk {
    /// Which algorithm this key is for. `ES256`, and a key claiming anything
    /// else is not one we can check a signature with.
    pub alg: String,

    /// The curve, `P-256`. Checked rather than assumed: a key on another curve
    /// with the same `alg` would be a confusion worth refusing.
    pub crv: String,

    /// Which key this is, and the id it was asked for by.
    pub kid: String,

    pub kty: String,

    /// The x coordinate, base64url, unpadded.
    pub x: String,

    /// The y coordinate, base64url, unpadded.
    pub y: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape Plaid sends, trimmed to what is read.
    const KEY: &str = r#"{
        "key": {
            "alg": "ES256",
            "created_at": 1234567890,
            "crv": "P-256",
            "expired_at": null,
            "kid": "6c5516e1-92dc-479e-a8ff-5a51992e0001",
            "kty": "EC",
            "use": "sig",
            "x": "sTIrM4RRPPS1Xy8Q6JQzMcaXqPPXFXNQIiOOKeGaKa8",
            "y": "AmDlwBcOTfLl1CTAWFVWEkg-3Cd2SS7hRvIULsyQpDs"
        },
        "request_id": "req-1"
    }"#;

    #[test]
    fn a_key_parses_to_its_two_coordinates() {
        let res: WebhookKeyResponse = serde_json::from_str(KEY).unwrap();

        assert_eq!(res.key.alg, "ES256");
        assert_eq!(res.key.crv, "P-256");
        assert_eq!(res.key.kid, "6c5516e1-92dc-479e-a8ff-5a51992e0001");
        assert!(!res.key.x.is_empty());
        assert!(!res.key.y.is_empty());
    }
}
