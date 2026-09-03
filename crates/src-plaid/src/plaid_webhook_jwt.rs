//! The token on a webhook, and the four things it has to survive.
//!
//! Pure: no clock, no network. The time and the key are handed in, so the
//! whole check is a function of its arguments and a test needs neither.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::Signature;
use p256::ecdsa::VerifyingKey;
use p256::ecdsa::signature::Verifier;
use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::PlaidJwk;
use crate::PlaidRejection;

/// How long after it was issued a webhook is still worth acting on.
///
/// Five minutes, which is Plaid's own suggestion. It is what stops a webhook
/// somebody captured being sent again tomorrow: the signature stays valid
/// forever, so the only thing that expires is the claim about when it was
/// made.
pub const SIGNED_GOOD_FOR_SECS: i64 = 5 * 60;

/// The header Plaid puts the token in. HTTP header names are case-insensitive,
/// so this is the spelling and not the match.
pub const VERIFICATION_HEADER: &str = "plaid-verification";

/// A webhook's JWT, taken apart but not yet believed.
///
/// Parsing and verifying are two steps on purpose. Parsing is free and needs
/// nothing from outside; verifying needs the key named by
/// [`kid`](PlaidWebhookJwt::kid), and fetching that key is an outbound call
/// caused by an anonymous POST. So the `alg` check happens in
/// [`parse`](PlaidWebhookJwt::parse) — before anybody has been asked for
/// anything — and a token that fails it costs a base64 decode.
pub struct PlaidWebhookJwt {
    /// `header.payload`, the bytes the signature is over. Kept as it arrived:
    /// re-encoding either half would change them.
    signed: String,

    /// Which key signed it, from the JWT header.
    kid: String,

    /// The raw signature, `r` and `s` concatenated.
    signature: Vec<u8>,

    /// The claims, still unverified — nothing reads them until the signature
    /// has been checked.
    payload: Vec<u8>,
}

/// The JWT header, as far as a verifier cares.
#[derive(Debug, Deserialize)]
struct JwtHeader {
    alg: String,

    #[serde(default)]
    kid: Option<String>,
}

/// The two claims Plaid signs.
#[derive(Debug, Deserialize)]
struct JwtClaims {
    /// When Plaid made the token, in seconds. The replay window is measured
    /// against this and nothing else — the signature itself never expires.
    iat: i64,

    /// The SHA-256 of the body Plaid sent, in lowercase hex.
    request_body_sha256: String,
}

impl PlaidWebhookJwt {
    /// Take a token apart, and refuse anything not signed with ES256.
    ///
    /// The algorithm check is here rather than in
    /// [`verify`](PlaidWebhookJwt::verify) because of what sits between the
    /// two: a key fetch. `alg: none` is the oldest hole in JWT and a symmetric
    /// `alg` is the next oldest, and both are closed by refusing before the
    /// token has caused any work at all.
    pub fn parse(token: &str) -> Result<Self, PlaidRejection> {
        let mut parts = token.trim().split('.');

        let (Some(header), Some(claims), Some(signature), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(PlaidRejection::Malformed("expected three parts"));
        };

        let decoded = decode(header).ok_or(PlaidRejection::Malformed("header is not base64url"))?;

        let decoded: JwtHeader = serde_json::from_slice(&decoded)
            .map_err(|_| PlaidRejection::Malformed("header is not JSON"))?;

        if decoded.alg != "ES256" {
            return Err(PlaidRejection::Algorithm(decoded.alg));
        }

        let kid = decoded.kid.filter(|kid| !kid.is_empty());

        let Some(kid) = kid else {
            return Err(PlaidRejection::Unkeyed);
        };

        Ok(Self {
            signed: format!("{header}.{claims}"),
            kid,
            signature: decode(signature)
                .ok_or(PlaidRejection::Malformed("signature is not base64url"))?,
            payload: decode(claims).ok_or(PlaidRejection::Malformed("claims are not base64url"))?,
        })
    }

    /// Which key this was signed with. The only thing a caller may read before
    /// [`verify`](PlaidWebhookJwt::verify) has passed, and it is a key id
    /// rather than anything the webhook asserts about a bank.
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// Signature, then freshness, then the body — in that order.
    ///
    /// The order is the whole design. Nothing about the claims is trusted
    /// until the signature has been checked against the key, so `iat` is read
    /// after and not before; and the body is compared last because it is the
    /// only check that needs anything but the token.
    ///
    /// `body` must be the **bytes as they arrived**. Plaid hashes what it put
    /// on the wire, which is pretty-printed with two spaces, so a body that
    /// has been parsed and serialised again hashes to something else entirely
    /// and every webhook is refused for a reason that looks like an attack.
    pub fn verify(&self, key: &PlaidJwk, body: &[u8], now: i64) -> Result<(), PlaidRejection> {
        let key = verifying_key(key, &self.kid)?;

        let signature =
            Signature::from_slice(&self.signature).map_err(|_| PlaidRejection::Signature)?;

        key.verify(self.signed.as_bytes(), &signature)
            .map_err(|_| PlaidRejection::Signature)?;

        let claims: JwtClaims =
            serde_json::from_slice(&self.payload).map_err(|_| PlaidRejection::Body)?;

        // Both directions. Late is a replay; early is a clock that has drifted
        // far enough that "five minutes old" has stopped meaning anything.
        let age = now - claims.iat;

        if age.abs() > SIGNED_GOOD_FOR_SECS {
            return Err(PlaidRejection::Stale { seconds: age });
        }

        let claimed = unhex(&claims.request_body_sha256).ok_or(PlaidRejection::Body)?;

        let actual = Sha256::digest(body);

        // Constant time, because the alternative leaks how much of a guessed
        // hash was right, one byte at a time.
        if actual.ct_eq(claimed.as_slice()).into() {
            return Ok(());
        }

        Err(PlaidRejection::Body)
    }
}

/// The JWK as something that can check a signature.
///
/// Every field is checked rather than assumed. A key that says `RS256`, or
/// sits on another curve, or came back under a different `kid` from the one
/// asked for, is a disagreement — and a verifier that carries on regardless is
/// one that can be talked into checking the wrong thing.
fn verifying_key(key: &PlaidJwk, kid: &str) -> Result<VerifyingKey, PlaidRejection> {
    if key.alg != "ES256" || key.crv != "P-256" || key.kty != "EC" || key.kid != kid {
        return Err(PlaidRejection::Unusable);
    }

    let (Some(x), Some(y)) = (decode(&key.x), decode(&key.y)) else {
        return Err(PlaidRejection::Unusable);
    };

    if x.len() != 32 || y.len() != 32 {
        return Err(PlaidRejection::Unusable);
    }

    // SEC1, uncompressed: the tag byte and then the two coordinates, which is
    // the form a JWK's halves reassemble into.
    let mut point = Vec::with_capacity(65);
    point.push(0x04);
    point.extend_from_slice(&x);
    point.extend_from_slice(&y);

    VerifyingKey::from_sec1_bytes(&point).map_err(|_| PlaidRejection::Unusable)
}

fn decode(value: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(value).ok()
}

/// Lowercase hex to bytes, exactly, or nothing.
///
/// Its own four lines rather than a dependency: this parses one field of one
/// header, and the failure case is "refuse the webhook".
fn unhex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }

    value
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(bytes: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(bytes)
    }

    fn token(header: &str) -> String {
        format!(
            "{}.{}.{}",
            encode(header.as_bytes()),
            encode(b"{}"),
            encode(&[0u8; 64])
        )
    }

    #[test]
    fn an_es256_token_comes_apart() {
        let jwt = PlaidWebhookJwt::parse(&token(r#"{"alg":"ES256","kid":"key-1"}"#)).unwrap();

        assert_eq!(jwt.kid(), "key-1");
    }

    /// The check that has to happen before a key is ever asked for.
    #[test]
    fn every_other_algorithm_is_refused() {
        for alg in ["none", "HS256", "RS256", "ES384", "es256"] {
            let parsed = PlaidWebhookJwt::parse(&token(&format!(r#"{{"alg":"{alg}","kid":"k"}}"#)));

            assert_eq!(
                parsed.err(),
                Some(PlaidRejection::Algorithm(alg.to_string())),
                "`{alg}` must not be accepted",
            );
        }
    }

    #[test]
    fn a_token_with_no_key_names_nothing_to_fetch() {
        let parsed = PlaidWebhookJwt::parse(&token(r#"{"alg":"ES256"}"#));

        assert_eq!(parsed.err(), Some(PlaidRejection::Unkeyed));
    }

    #[test]
    fn anything_that_is_not_three_parts_is_malformed() {
        for value in ["", "a.b", "a.b.c.d", "not-a-token"] {
            assert!(
                PlaidWebhookJwt::parse(value).is_err(),
                "`{value}` must not parse",
            );
        }
    }

    /// A key we hold both halves of, so a test can sign what it verifies.
    ///
    /// Fixed rather than drawn: ECDSA here is deterministic (RFC 6979), so
    /// with a fixed key and a fixed message the whole test is reproducible.
    fn signing_key(scalar: u8) -> p256::ecdsa::SigningKey {
        let mut bytes = [0u8; 32];
        bytes[31] = scalar;

        p256::ecdsa::SigningKey::from_bytes(&bytes.into()).expect("a small scalar is a valid key")
    }

    /// The same key as a JWK, which is the form Plaid hands one over in.
    fn jwk(kid: &str, scalar: u8) -> PlaidJwk {
        let point = signing_key(scalar).verifying_key().to_encoded_point(false);

        PlaidJwk {
            alg: "ES256".to_string(),
            crv: "P-256".to_string(),
            kid: kid.to_string(),
            kty: "EC".to_string(),
            x: encode(point.x().expect("an uncompressed point has an x")),
            y: encode(point.y().expect("an uncompressed point has a y")),
        }
    }

    /// A token over `body`, signed the way Plaid signs one.
    fn signed(kid: &str, body: &[u8], iat: i64) -> String {
        use p256::ecdsa::signature::Signer;

        let header = encode(format!(r#"{{"alg":"ES256","kid":"{kid}","typ":"JWT"}}"#).as_bytes());

        let hash = Sha256::digest(body);
        let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();

        let claims =
            encode(format!(r#"{{"iat":{iat},"request_body_sha256":"{hash}"}}"#).as_bytes());

        let signing = format!("{header}.{claims}");

        let signature: Signature = signing_key(SIGNER).sign(signing.as_bytes());

        format!("{signing}.{}", encode(&signature.to_bytes()))
    }

    /// The body as Plaid sends it: pretty-printed, two spaces. Hashing
    /// anything else — including this parsed and printed again — is the
    /// mistake this whole check exists to catch.
    const BODY: &[u8] =
        b"{\n  \"webhook_type\": \"LINK\",\n  \"webhook_code\": \"SESSION_FINISHED\"\n}";

    const NOW: i64 = 1_787_097_600;

    /// The key every token here is signed with, and the one every test but
    /// [`a_signature_from_another_key_is_refused`] verifies against.
    const SIGNER: u8 = 42;

    #[test]
    fn a_signed_webhook_is_believed() {
        let jwt = PlaidWebhookJwt::parse(&signed("key-1", BODY, NOW)).unwrap();

        assert_eq!(jwt.verify(&jwk("key-1", SIGNER), BODY, NOW), Ok(()));
    }

    /// A minute of clock drift is nothing; a token from this morning is a
    /// replay. The signature is good in both cases, which is the point.
    #[test]
    fn only_a_recent_token_is_believed() {
        let jwt = PlaidWebhookJwt::parse(&signed("key-1", BODY, NOW)).unwrap();
        let key = jwk("key-1", SIGNER);

        assert_eq!(jwt.verify(&key, BODY, NOW + 60), Ok(()));

        assert_eq!(
            jwt.verify(&key, BODY, NOW + SIGNED_GOOD_FOR_SECS + 1),
            Err(PlaidRejection::Stale {
                seconds: SIGNED_GOOD_FOR_SECS + 1,
            }),
        );
    }

    /// The single most common way this integration is got wrong: the hash is
    /// over the bytes that arrived, and a body that has been through serde is
    /// not those bytes.
    #[test]
    fn a_body_that_was_reserialised_no_longer_matches() {
        let jwt = PlaidWebhookJwt::parse(&signed("key-1", BODY, NOW)).unwrap();

        let value: serde_json::Value = serde_json::from_slice(BODY).unwrap();
        let printed = serde_json::to_vec(&value).unwrap();

        assert_ne!(printed, BODY, "the fixture must survive being reprinted");

        assert_eq!(
            jwt.verify(&jwk("key-1", SIGNER), &printed, NOW),
            Err(PlaidRejection::Body),
        );
    }

    /// Somebody else signed it. A real key, a real signature, and not ours —
    /// which is the shape a forgery actually takes.
    #[test]
    fn a_signature_from_another_key_is_refused() {
        let jwt = PlaidWebhookJwt::parse(&signed("key-1", BODY, NOW)).unwrap();

        assert_eq!(
            jwt.verify(&jwk("key-1", SIGNER + 1), BODY, NOW),
            Err(PlaidRejection::Signature),
        );
    }

    /// The key came back under another name than the one asked for, so it is
    /// not the key this token names.
    #[test]
    fn a_key_for_another_kid_is_not_used() {
        let jwt = PlaidWebhookJwt::parse(&signed("key-1", BODY, NOW)).unwrap();

        assert_eq!(
            jwt.verify(&jwk("key-2", SIGNER), BODY, NOW),
            Err(PlaidRejection::Unusable),
        );
    }

    #[test]
    fn hex_decodes_or_refuses() {
        assert_eq!(unhex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }
}
