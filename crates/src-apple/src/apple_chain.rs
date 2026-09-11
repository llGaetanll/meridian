//! The walk from the root we trust down to the certificate that signed.
//!
//! Pure: no clock, no network. The time and the root are handed in, so the
//! whole check is a function of its arguments and a test needs neither.

use x509_cert::Certificate;
use x509_cert::der::Decode;
use x509_cert::der::Encode;
use x509_cert::der::asn1::ObjectIdentifier;
use x509_cert::ext::pkix::BasicConstraints;

use crate::AppleKey;
use crate::AppleRejection;

/// Whether a certificate may issue another one.
const BASIC_CONSTRAINTS: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.19");

/// Apple's mark on a certificate that signs for the App Store.
///
/// The check that makes the difference between "Apple issued this" and "Apple
/// issued this *for signing transactions*". Any developer can get a
/// certificate under Apple's root — a push certificate, an Apple Pay merchant
/// identity — and every one of them would otherwise chain just as well as the
/// real thing.
const APP_STORE_SIGNING: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113635.100.6.11.1");

/// Apple's mark on the Worldwide Developer Relations authority, which is what
/// issues the leaf.
///
/// The trap: if Apple ever moves App Store signing under another of its
/// authorities, every payload starts being refused here and nothing else
/// changes. Apple's own verifiers make the same check, so the answer is the
/// same as theirs — the new intermediate's identifier goes in.
const WWDR_AUTHORITY: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113635.100.6.2.1");

/// The key of the certificate at the end of `chain`, once every step from
/// `root` down to it has held.
///
/// `chain` is the header's `x5c` as Apple sends it — the leaf first, then the
/// authority that issued it, then Apple's root. **The root it carries is not
/// trusted for being there.** The walk starts at `root`, which is ours, and a
/// chain ending in anything else fails at the first signature it is asked for.
///
/// Each step is four things: the signature against the key above it, the
/// validity window against `now`, that an issuing certificate says it may
/// issue, and that the leaf says what it is for.
pub fn walk(chain: &[Vec<u8>], root: &[u8], now: i64) -> Result<AppleKey, AppleRejection> {
    if chain.is_empty() {
        return Err(AppleRejection::Unchained);
    }

    let root =
        Certificate::from_der(root).map_err(|_| AppleRejection::Certificate("the pinned root"))?;

    let mut issuer = AppleKey::read(&root.tbs_certificate.subject_public_key_info)?;
    let mut depth = chain.len();

    for encoded in chain.iter().rev() {
        depth -= 1;

        let certificate = Certificate::from_der(encoded.as_slice())
            .map_err(|_| AppleRejection::Certificate("one in the chain"))?;

        // The bytes that were signed are the certificate's own re-encoding of
        // what it holds, not the slice it arrived in.
        let signed = certificate
            .tbs_certificate
            .to_der()
            .map_err(|_| AppleRejection::Certificate("will not re-encode"))?;

        let signature = certificate
            .signature
            .as_bytes()
            .ok_or(AppleRejection::Certificate("a signature of partial bytes"))?;

        issuer.signed_certificate(&signed, signature, &certificate.signature_algorithm.oid)?;

        within(&certificate, now)?;

        match depth {
            0 => {
                if extension(&certificate, &APP_STORE_SIGNING).is_none() {
                    return Err(AppleRejection::Purpose);
                }
            }

            _ => {
                if !issues(&certificate) {
                    return Err(AppleRejection::NotACa);
                }

                if depth == 1 && extension(&certificate, &WWDR_AUTHORITY).is_none() {
                    return Err(AppleRejection::Purpose);
                }
            }
        }

        issuer = AppleKey::read(&certificate.tbs_certificate.subject_public_key_info)?;
    }

    Ok(issuer)
}

/// Whether `now` is inside what the certificate is good for, in either
/// direction. A certificate from the future is as much a forgery as an expired
/// one.
fn within(certificate: &Certificate, now: i64) -> Result<(), AppleRejection> {
    let validity = &certificate.tbs_certificate.validity;

    let (Ok(from), Ok(until)) = (
        i64::try_from(validity.not_before.to_unix_duration().as_secs()),
        i64::try_from(validity.not_after.to_unix_duration().as_secs()),
    ) else {
        return Err(AppleRejection::Certificate("a validity beyond the epoch"));
    };

    if (from..=until).contains(&now) {
        Ok(())
    } else {
        Err(AppleRejection::Expired)
    }
}

/// Whether a certificate says it may sign another one.
///
/// Absent is not an authority, and neither is one that says so without saying
/// `ca`. This is what stops an ordinary Apple-issued certificate from being
/// used as an authority by whoever holds its key.
fn issues(certificate: &Certificate) -> bool {
    extension(certificate, &BASIC_CONSTRAINTS)
        .and_then(|value| BasicConstraints::from_der(value).ok())
        .is_some_and(|constraints| constraints.ca)
}

/// One extension's bytes, by identifier.
fn extension<'a>(certificate: &'a Certificate, oid: &ObjectIdentifier) -> Option<&'a [u8]> {
    certificate
        .tbs_certificate
        .extensions
        .as_ref()?
        .iter()
        .find(|extension| extension.extn_id == *oid)
        .map(|extension| extension.extn_value.as_bytes())
}
