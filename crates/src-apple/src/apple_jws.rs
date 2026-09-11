//! One payload Apple signed, and what it has to survive to be read.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;

use crate::AppleRejection;
use crate::apple_chain;

/// A signed payload, taken apart but not yet believed.
///
/// Parsing and verifying are two steps so that the cheap refusals happen
/// first. The algorithm and the shape of the header cost a base64 decode; the
/// chain costs three signature checks, and nothing should be able to make this
/// server do those by sending it a token that was never going to be read.
struct AppleJws {
    /// `header.payload`, the bytes the signature is over. Kept as it arrived:
    /// re-encoding either half would change them.
    signed: String,

    /// The certificates from the header, leaf first, as DER.
    chain: Vec<Vec<u8>>,

    /// The raw signature, `r` and `s` concatenated.
    signature: Vec<u8>,

    /// The payload, still unbelieved — nothing reads it until the chain and
    /// the signature have held.
    payload: Vec<u8>,
}

/// The JWS header, as far as a verifier cares.
#[derive(Debug, Deserialize)]
struct JwsHeader {
    alg: String,

    /// The chain that signed this, each certificate base64 with its padding —
    /// which is not the base64url the rest of a JWS is in.
    #[serde(default)]
    x5c: Vec<String>,
}

/// The payload of something Apple signed, once nothing is left to doubt.
///
/// `root` is the trust anchor, which in every caller but a test is
/// [`APPLE_ROOT_CA_G3`](crate::APPLE_ROOT_CA_G3). `now` is the tick's, so a
/// replay lands where it did the first time, and it is used for one thing: the
/// validity windows of the certificates.
///
/// **There is no replay window on the payload**, as there is on a Plaid
/// webhook. The payload carries a `signedDate` and this does not read it:
/// nothing is decided here, and a payload that arrives twice names the same
/// transaction both times, so acting on it again writes the row it wrote the
/// first time. Whoever reads the payload owns that rule, and the certificates
/// are the only thing `now` is spent on.
pub fn verify(token: &str, root: &[u8], now: i64) -> Result<Vec<u8>, AppleRejection> {
    let jws = AppleJws::parse(token)?;

    let leaf = apple_chain::walk(&jws.chain, root, now)?;

    leaf.signed_payload(jws.signed.as_bytes(), &jws.signature)?;

    Ok(jws.payload)
}

impl AppleJws {
    /// Take a token apart, and refuse anything not signed with ES256.
    fn parse(token: &str) -> Result<Self, AppleRejection> {
        let mut parts = token.trim().split('.');

        let (Some(header), Some(payload), Some(signature), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(AppleRejection::Malformed("expected three parts"));
        };

        let decoded = decode(header).ok_or(AppleRejection::Malformed("header is not base64url"))?;

        let decoded: JwsHeader = serde_json::from_slice(&decoded)
            .map_err(|_| AppleRejection::Malformed("header is not JSON"))?;

        if decoded.alg != "ES256" {
            return Err(AppleRejection::Algorithm(decoded.alg));
        }

        if decoded.x5c.is_empty() {
            return Err(AppleRejection::Unchained);
        }

        let chain = decoded
            .x5c
            .iter()
            .map(|certificate| STANDARD.decode(certificate).ok())
            .collect::<Option<Vec<_>>>()
            .ok_or(AppleRejection::Certificate("not base64"))?;

        Ok(Self {
            signed: format!("{header}.{payload}"),
            chain,
            signature: decode(signature)
                .ok_or(AppleRejection::Malformed("signature is not base64url"))?,
            payload: decode(payload)
                .ok_or(AppleRejection::Malformed("payload is not base64url"))?,
        })
    }
}

fn decode(value: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(value).ok()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::time::Duration;

    use p256::ecdsa::signature::Signer;
    use x509_cert::builder::Builder;
    use x509_cert::builder::CertificateBuilder;
    use x509_cert::builder::Profile;
    use x509_cert::der::Encode;
    use x509_cert::der::Length;
    use x509_cert::der::Writer;
    use x509_cert::der::asn1::Null;
    use x509_cert::der::asn1::ObjectIdentifier;
    use x509_cert::der::asn1::UtcTime;
    use x509_cert::der::oid::AssociatedOid;
    use x509_cert::ext::AsExtension;
    use x509_cert::ext::Extension;
    use x509_cert::name::Name;
    use x509_cert::serial_number::SerialNumber;
    use x509_cert::spki::SubjectPublicKeyInfoOwned;
    use x509_cert::time::Time;
    use x509_cert::time::Validity;

    use super::*;
    use crate::APPLE_ROOT_CA_G3;

    /// Apple's mark on a certificate, which is an extension whose identifier
    /// is the whole of what it says. The builder wants a type per identifier,
    /// so there are two of them and they differ in nothing else.
    macro_rules! marker {
        ($name:ident, $oid:literal) => {
            struct $name;

            impl AssociatedOid for $name {
                const OID: ObjectIdentifier = ObjectIdentifier::new_unwrap($oid);
            }

            impl Encode for $name {
                fn encoded_len(&self) -> x509_cert::der::Result<Length> {
                    Null.encoded_len()
                }

                fn encode(&self, writer: &mut impl Writer) -> x509_cert::der::Result<()> {
                    Null.encode(writer)
                }
            }

            impl AsExtension for $name {
                fn critical(&self, _subject: &Name, _extensions: &[Extension]) -> bool {
                    false
                }
            }
        };
    }

    marker!(AppStoreSigning, "1.2.840.113635.100.6.11.1");
    marker!(WwdrAuthority, "1.2.840.113635.100.6.2.1");

    /// The moment every certificate here is good at, and the one every test
    /// verifies at unless it is about time.
    const NOW: i64 = 1_787_097_600;

    const YEAR: i64 = 365 * 24 * 60 * 60;

    /// What a test wants wrong with the chain it builds.
    enum Flaw {
        /// Nothing: the shape Apple sends.
        None,

        /// A leaf that does not say it signs for the App Store.
        UnmarkedLeaf,

        /// An authority that does not say it is Apple's.
        UnmarkedAuthority,

        /// An ordinary certificate in the place of the authority — which is
        /// what somebody holding any Apple-issued certificate would try.
        NotAnAuthority,
    }

    /// A chain of our own in the shape Apple's is: a root, the authority it
    /// issued, and the leaf that signs a payload.
    struct Chain {
        /// The root, as the trust anchor a verifier is handed.
        root: Vec<u8>,

        /// The same three certificates as the header carries them, leaf first.
        x5c: Vec<String>,

        /// The leaf's other half, so a test can sign what it verifies.
        leaf: p256::ecdsa::SigningKey,
    }

    /// Keys we hold both halves of.
    ///
    /// Fixed rather than drawn: ECDSA here is deterministic (RFC 6979), so a
    /// chain built from these is the same bytes on every run.
    fn signing_key_384(scalar: u8) -> p384::ecdsa::SigningKey {
        let mut bytes = [0u8; 48];
        bytes[47] = scalar;

        p384::ecdsa::SigningKey::from_bytes(&bytes.into()).expect("a small scalar is a valid key")
    }

    fn signing_key_256(scalar: u8) -> p256::ecdsa::SigningKey {
        let mut bytes = [0u8; 32];
        bytes[31] = scalar;

        p256::ecdsa::SigningKey::from_bytes(&bytes.into()).expect("a small scalar is a valid key")
    }

    fn name(common: &str) -> Name {
        Name::from_str(&format!("CN={common},O=Not Apple,C=US")).unwrap()
    }

    /// A year either side of [`NOW`], so a test about time has somewhere to
    /// stand in both directions.
    fn validity() -> Validity {
        let at = |secs: i64| {
            Time::UtcTime(UtcTime::from_unix_duration(Duration::from_secs(secs as u64)).unwrap())
        };

        Validity {
            not_before: at(NOW - YEAR),
            not_after: at(NOW + YEAR),
        }
    }

    fn public_384(key: &p384::ecdsa::SigningKey) -> SubjectPublicKeyInfoOwned {
        SubjectPublicKeyInfoOwned::from_key(*key.verifying_key()).unwrap()
    }

    fn chain(flaw: Flaw) -> Chain {
        let root_key = signing_key_384(1);
        let authority_key = signing_key_384(2);
        let leaf_key = signing_key_256(3);

        let root = CertificateBuilder::new(
            Profile::Root,
            SerialNumber::from(1u32),
            validity(),
            name("Root"),
            public_384(&root_key),
            &root_key,
        )
        .unwrap()
        .build::<p384::ecdsa::DerSignature>()
        .unwrap();

        let profile = match flaw {
            Flaw::NotAnAuthority => Profile::Leaf {
                issuer: name("Root"),
                enable_key_agreement: false,
                enable_key_encipherment: false,
            },

            _ => Profile::SubCA {
                issuer: name("Root"),
                path_len_constraint: None,
            },
        };

        let mut builder = CertificateBuilder::new(
            profile,
            SerialNumber::from(2u32),
            validity(),
            name("Authority"),
            public_384(&authority_key),
            &root_key,
        )
        .unwrap();

        if !matches!(flaw, Flaw::UnmarkedAuthority) {
            builder.add_extension(&WwdrAuthority).unwrap();
        }

        let authority = builder.build::<p384::ecdsa::DerSignature>().unwrap();

        let mut builder = CertificateBuilder::new(
            Profile::Leaf {
                issuer: name("Authority"),
                enable_key_agreement: false,
                enable_key_encipherment: false,
            },
            SerialNumber::from(3u32),
            validity(),
            name("Leaf"),
            SubjectPublicKeyInfoOwned::from_key(*leaf_key.verifying_key()).unwrap(),
            &authority_key,
        )
        .unwrap();

        if !matches!(flaw, Flaw::UnmarkedLeaf) {
            builder.add_extension(&AppStoreSigning).unwrap();
        }

        let leaf = builder.build::<p384::ecdsa::DerSignature>().unwrap();

        Chain {
            root: root.to_der().unwrap(),
            x5c: [leaf, authority, root]
                .iter()
                .map(|certificate| STANDARD.encode(certificate.to_der().unwrap()))
                .collect(),
            leaf: leaf_key,
        }
    }

    /// A payload signed the way Apple signs one.
    fn token(chain: &Chain, payload: &str) -> String {
        let x5c = chain
            .x5c
            .iter()
            .map(|certificate| format!("\"{certificate}\""))
            .collect::<Vec<_>>()
            .join(",");

        let header = URL_SAFE_NO_PAD.encode(format!(r#"{{"alg":"ES256","x5c":[{x5c}]}}"#));
        let signed = format!("{header}.{}", URL_SAFE_NO_PAD.encode(payload));

        let signature: p256::ecdsa::Signature = chain.leaf.sign(signed.as_bytes());

        format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
    }

    const PAYLOAD: &str = r#"{"transactionId":"2000000000000001"}"#;

    #[test]
    fn a_signed_payload_is_read() {
        let chain = chain(Flaw::None);

        assert_eq!(
            verify(&token(&chain, PAYLOAD), &chain.root, NOW),
            Ok(PAYLOAD.as_bytes().to_vec()),
        );
    }

    /// The chain is Apple's and the payload is not: somebody kept the header
    /// and wrote their own transaction underneath it.
    #[test]
    fn a_payload_that_was_edited_is_refused() {
        let chain = chain(Flaw::None);

        let token = token(&chain, PAYLOAD).replace(
            &URL_SAFE_NO_PAD.encode(PAYLOAD),
            &URL_SAFE_NO_PAD.encode(r#"{"transactionId":"2000000000000002"}"#),
        );

        assert_eq!(
            verify(&token, &chain.root, NOW),
            Err(AppleRejection::Signature)
        );
    }

    /// The whole chain verifies, against a root that is not the one we trust.
    /// This is what every forgery that does not hold an Apple key looks like.
    #[test]
    fn a_chain_that_does_not_reach_apples_root_is_refused() {
        let chain = chain(Flaw::None);

        assert_eq!(
            verify(&token(&chain, PAYLOAD), APPLE_ROOT_CA_G3, NOW),
            Err(AppleRejection::Untrusted),
        );
    }

    /// A certificate Apple issued for something else, used to sign a
    /// transaction. Any developer can have one.
    #[test]
    fn a_leaf_that_does_not_sign_for_the_app_store_is_refused() {
        let chain = chain(Flaw::UnmarkedLeaf);

        assert_eq!(
            verify(&token(&chain, PAYLOAD), &chain.root, NOW),
            Err(AppleRejection::Purpose),
        );
    }

    /// The same one step up: an ordinary certificate signing a leaf of its
    /// own. The arithmetic comes out right and it is still not an authority.
    #[test]
    fn an_issuer_that_may_not_issue_is_refused() {
        let chain = chain(Flaw::NotAnAuthority);

        assert_eq!(
            verify(&token(&chain, PAYLOAD), &chain.root, NOW),
            Err(AppleRejection::NotACa),
        );
    }

    #[test]
    fn an_authority_that_is_not_apples_is_refused() {
        let chain = chain(Flaw::UnmarkedAuthority);

        assert_eq!(
            verify(&token(&chain, PAYLOAD), &chain.root, NOW),
            Err(AppleRejection::Purpose),
        );
    }

    /// Both directions. A certificate that has run out and one that has not
    /// started are the same refusal.
    #[test]
    fn a_certificate_outside_its_window_is_refused() {
        let chain = chain(Flaw::None);
        let token = token(&chain, PAYLOAD);

        assert_eq!(
            verify(&token, &chain.root, NOW + YEAR + 1),
            Err(AppleRejection::Expired),
        );

        assert_eq!(
            verify(&token, &chain.root, NOW - YEAR - 1),
            Err(AppleRejection::Expired),
        );
    }

    /// The check that happens before a single certificate is parsed.
    #[test]
    fn every_other_algorithm_is_refused() {
        let chain = chain(Flaw::None);

        for alg in ["none", "HS256", "RS256", "ES384", "es256"] {
            let header =
                URL_SAFE_NO_PAD.encode(format!(r#"{{"alg":"{alg}","x5c":["{}"]}}"#, chain.x5c[0]));

            let token = format!("{header}.{}.{}", URL_SAFE_NO_PAD.encode(PAYLOAD), "x");

            assert_eq!(
                verify(&token, &chain.root, NOW),
                Err(AppleRejection::Algorithm(alg.to_string())),
                "`{alg}` must not be accepted",
            );
        }
    }

    #[test]
    fn a_token_carrying_no_chain_is_refused() {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#);
        let token = format!("{header}.{}.{}", URL_SAFE_NO_PAD.encode(PAYLOAD), "x");

        assert_eq!(
            verify(&token, APPLE_ROOT_CA_G3, NOW),
            Err(AppleRejection::Unchained),
        );
    }

    #[test]
    fn anything_that_is_not_three_parts_is_malformed() {
        for value in ["", "a.b", "a.b.c.d", "not-a-token"] {
            assert!(
                verify(value, APPLE_ROOT_CA_G3, NOW).is_err(),
                "`{value}` must not verify",
            );
        }
    }
}
