//! A public key taken out of a certificate, and what it will vouch for.

use p256::ecdsa::signature::Verifier;
use x509_cert::der::asn1::ObjectIdentifier;
use x509_cert::spki::SubjectPublicKeyInfoOwned;

use crate::AppleRejection;

/// Elliptic curve keys, which is all Apple signs with.
const EC_PUBLIC_KEY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");

/// The two curves in Apple's chain: the leaf is on P-256 and everything above
/// it on P-384. Each is written into a `const`, so an identifier that is not
/// one fails to compile rather than to parse.
const P256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
const P384: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.132.0.34");

/// The signature algorithms that go with them, one hash per curve.
const ECDSA_SHA256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.2");
const ECDSA_SHA384: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.3");

/// A key that may be asked whether it signed something.
///
/// Holding the curve in the type is what keeps the hash from being a choice:
/// a P-256 key verifies over SHA-256 and a P-384 key over SHA-384, because
/// that is what each curve's `Verifier` does. Nothing here reads an algorithm
/// and then picks a digest, which is the shape that lets a caller be talked
/// into the wrong one.
#[derive(Debug)]
pub enum AppleKey {
    P256(p256::ecdsa::VerifyingKey),
    P384(p384::ecdsa::VerifyingKey),
}

impl AppleKey {
    /// The key a certificate carries.
    ///
    /// Every field is checked rather than assumed: a key that is not on a
    /// curve, or is on one Apple does not use, is refused here rather than
    /// being made to fit.
    pub fn read(spki: &SubjectPublicKeyInfoOwned) -> Result<Self, AppleRejection> {
        if spki.algorithm.oid != EC_PUBLIC_KEY {
            return Err(AppleRejection::Certificate("the key is not on a curve"));
        }

        let curve = spki
            .algorithm
            .parameters
            .as_ref()
            .and_then(|parameters| parameters.decode_as::<ObjectIdentifier>().ok())
            .ok_or(AppleRejection::Certificate("the key names no curve"))?;

        let point = spki
            .subject_public_key
            .as_bytes()
            .ok_or(AppleRejection::Certificate("the key is not whole bytes"))?;

        match curve {
            P256 => p256::ecdsa::VerifyingKey::from_sec1_bytes(point)
                .map(AppleKey::P256)
                .map_err(|_| AppleRejection::Certificate("the key is not a point on P-256")),

            P384 => p384::ecdsa::VerifyingKey::from_sec1_bytes(point)
                .map(AppleKey::P384)
                .map_err(|_| AppleRejection::Certificate("the key is not a point on P-384")),

            _ => Err(AppleRejection::Certificate("the key is on another curve")),
        }
    }

    /// Whether this key signed a certificate, whose signature is DER.
    ///
    /// `algorithm` has to be the one that goes with this key's curve. A
    /// mismatch is refused rather than honoured: it is the only way the digest
    /// and the curve can come apart.
    pub fn signed_certificate(
        &self,
        message: &[u8],
        signature: &[u8],
        algorithm: &ObjectIdentifier,
    ) -> Result<(), AppleRejection> {
        match self {
            AppleKey::P256(key) => {
                if *algorithm != ECDSA_SHA256 {
                    return Err(AppleRejection::Algorithm(algorithm.to_string()));
                }

                let signature = p256::ecdsa::DerSignature::from_bytes(signature)
                    .map_err(|_| AppleRejection::Untrusted)?;

                key.verify(message, &signature)
                    .map_err(|_| AppleRejection::Untrusted)
            }

            AppleKey::P384(key) => {
                if *algorithm != ECDSA_SHA384 {
                    return Err(AppleRejection::Algorithm(algorithm.to_string()));
                }

                let signature = p384::ecdsa::DerSignature::from_bytes(signature)
                    .map_err(|_| AppleRejection::Untrusted)?;

                key.verify(message, &signature)
                    .map_err(|_| AppleRejection::Untrusted)
            }
        }
    }

    /// Whether this key signed a payload, whose signature is `r` and `s` laid
    /// end to end.
    ///
    /// Only P-256 answers. `ES256` names the curve as well as the hash, and a
    /// leaf on any other curve is one this payload never claimed to be signed
    /// by.
    pub fn signed_payload(&self, message: &[u8], signature: &[u8]) -> Result<(), AppleRejection> {
        let AppleKey::P256(key) = self else {
            return Err(AppleRejection::Certificate("the leaf is not on P-256"));
        };

        let signature =
            p256::ecdsa::Signature::from_slice(signature).map_err(|_| AppleRejection::Signature)?;

        key.verify(message, &signature)
            .map_err(|_| AppleRejection::Signature)
    }
}

#[cfg(test)]
mod tests {
    use x509_cert::Certificate;
    use x509_cert::der::Decode;
    use x509_cert::der::Encode;

    use super::*;
    use crate::APPLE_ROOT_CA_G3;

    /// Apple's Worldwide Developer Relations authority, which is what issues
    /// the leaf that signs a transaction. Test data and not a trust anchor:
    /// the only certificate this server believes on sight is the root.
    const APPLE_WWDR_G6: &[u8] = include_bytes!("../certs/AppleWWDRCAG6.cer");

    /// The one test here made of real bytes, and the only thing tying this
    /// code to Apple rather than to a chain we made up.
    ///
    /// It claims two things at once. The signature on Apple's authority checks
    /// out against Apple's root — P-384 over SHA-384, with the signature in
    /// DER, which is the pairing nothing else in this crate exercises. And the
    /// file behind [`APPLE_ROOT_CA_G3`] is the certificate it says it is,
    /// because no other root would produce this.
    #[test]
    fn apples_root_signed_apples_authority() {
        let root = Certificate::from_der(APPLE_ROOT_CA_G3).unwrap();
        let authority = Certificate::from_der(APPLE_WWDR_G6).unwrap();

        let key = AppleKey::read(&root.tbs_certificate.subject_public_key_info).unwrap();

        assert!(matches!(key, AppleKey::P384(_)), "Apple's root is on P-384");

        let signed = authority.tbs_certificate.to_der().unwrap();

        assert_eq!(
            key.signed_certificate(
                &signed,
                authority.signature.as_bytes().unwrap(),
                &authority.signature_algorithm.oid,
            ),
            Ok(()),
        );
    }

    /// The same signature against the same key, over bytes that are one bit
    /// different.
    #[test]
    fn a_certificate_that_was_edited_is_not_signed() {
        let root = Certificate::from_der(APPLE_ROOT_CA_G3).unwrap();
        let authority = Certificate::from_der(APPLE_WWDR_G6).unwrap();

        let key = AppleKey::read(&root.tbs_certificate.subject_public_key_info).unwrap();

        let mut signed = authority.tbs_certificate.to_der().unwrap();
        let last = signed.len() - 1;
        signed[last] ^= 1;

        assert_eq!(
            key.signed_certificate(
                &signed,
                authority.signature.as_bytes().unwrap(),
                &authority.signature_algorithm.oid,
            ),
            Err(AppleRejection::Untrusted),
        );
    }

    /// A P-384 key is never asked to check a SHA-256 signature, and a P-256
    /// key never a SHA-384 one. The digest follows the curve, and a payload
    /// claiming otherwise is refused rather than accommodated.
    #[test]
    fn a_signature_algorithm_that_does_not_go_with_the_key_is_refused() {
        let root = Certificate::from_der(APPLE_ROOT_CA_G3).unwrap();
        let authority = Certificate::from_der(APPLE_WWDR_G6).unwrap();

        let key = AppleKey::read(&root.tbs_certificate.subject_public_key_info).unwrap();

        let signed = authority.tbs_certificate.to_der().unwrap();

        assert_eq!(
            key.signed_certificate(
                &signed,
                authority.signature.as_bytes().unwrap(),
                &ECDSA_SHA256,
            ),
            Err(AppleRejection::Algorithm(ECDSA_SHA256.to_string())),
        );
    }
}
