//! The one column that is not stored as it was given.
//!
//! Plaid's access token never expires and reads somebody's bank account, so it
//! does not sit in the database in the clear. Everything here is about that one
//! column: the key, and the two directions.
//!
//! ## What this protects against, and what it does not
//!
//! A stolen volume, a leaked backup, a `pg_dump` pasted into a ticket, a
//! replica, anyone holding database credentials but not the server. That is how
//! this data usually escapes, and against all of it the column is noise.
//!
//! It is no protection whatever against somebody who has the server, because
//! the server has the key. This is one column rather than a secrets programme —
//! that is issue #16, and this is #21.
//!
//! ## The cost, which is real
//!
//! Lose the key and every access token is scrap: every bank has to be relinked,
//! and in Production relinking spends Item slots that do not come back. The key
//! needs backing up as carefully as the database does, and the two should not
//! be backed up to the same place, or a single theft has both.

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::Aead;
use aes_gcm::aead::AeadCore;
use aes_gcm::aead::KeyInit;
use aes_gcm::aead::OsRng;
use base64::Engine;

use crate::PostgresError;

/// What a sealed value carries in front of the ciphertext.
///
/// One byte, so that a blob found in the database says what it is rather than
/// being guessed at. What changes over time is the whole scheme — a new key, a
/// different algorithm, another nonce length — and a reader that cannot tell
/// which one it is holding has no way to open the old rows after a rotation.
/// Writing it now costs one byte; adding it later costs a guess per row.
const VERSION: u8 = 1;

/// AES-GCM's nonce is 96 bits. Not ours to choose.
const NONCE: usize = 12;

/// The key every access token is sealed with.
///
/// Read once, on the way up, and held — so a process without one fails at
/// startup rather than at the first bank connection, which is the difference
/// between a server that will not start and a server that is broken in a way
/// nobody notices until somebody tries to link a bank.
pub struct DbKey(aes_gcm::Key<Aes256Gcm>);

impl DbKey {
    /// The key named by `MERIDIAN_DB_KEY`: 32 random bytes, base64.
    ///
    /// No default, and nothing generated when it is missing. A default key is a
    /// key that ships; a key invented at startup would open nothing the last
    /// run wrote, which reads as data loss and is worse than refusing to start.
    pub fn from_env() -> Result<Self, PostgresError> {
        let value = std::env::var("MERIDIAN_DB_KEY").map_err(|_| {
            PostgresError::Key(String::from(
                "MERIDIAN_DB_KEY is not set — it is 32 random bytes, base64, \
                 and `scripts/db-init.sh` writes one",
            ))
        })?;

        Self::parse(&value)
    }

    /// Split from [`from_env`](DbKey::from_env) so the parse is testable
    /// without setting a variable in a process the whole test suite shares.
    pub fn parse(value: &str) -> Result<Self, PostgresError> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value.trim())
            .map_err(|_| {
                PostgresError::Key(String::from("MERIDIAN_DB_KEY is not valid base64"))
            })?;

        // The length is the whole of the validation. Anything that decodes to
        // 32 bytes is a key as far as AES is concerned, and a wrong one is
        // indistinguishable from a right one until something fails to open.
        let sized: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            PostgresError::Key(format!(
                "MERIDIAN_DB_KEY decodes to {} bytes — AES-256 needs exactly 32",
                bytes.len(),
            ))
        })?;

        Ok(Self(sized.into()))
    }
}

/// Seal a token for storage.
///
/// A fresh nonce every time, which is not a precaution but the condition
/// AES-GCM is safe under: reusing one with the same key leaks the plaintext of
/// both messages and the ability to forge. `generate_nonce` draws from the OS,
/// so nothing here counts or remembers.
pub(crate) fn seal(key: &DbKey, plain: &str) -> Result<Vec<u8>, PostgresError> {
    let cipher = Aes256Gcm::new(&key.0);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    // The error is deliberately bare. Anything more would be describing the
    // token, and this message reaches a log.
    let sealed = cipher
        .encrypt(&nonce, plain.as_bytes())
        .map_err(|_| PostgresError::Crypto("could not seal the access token"))?;

    let mut blob = Vec::with_capacity(1 + NONCE + sealed.len());
    blob.push(VERSION);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&sealed);

    Ok(blob)
}

/// Open one, or say why not.
///
/// Every failure here means the same thing operationally — this row cannot be
/// used and the bank has to be relinked — but they are told apart because they
/// have entirely different causes: a truncated blob is a bug in what wrote it,
/// and a tag that does not verify is the wrong key or a tampered row.
pub(crate) fn open(key: &DbKey, blob: &[u8]) -> Result<String, PostgresError> {
    let Some((&version, rest)) = blob.split_first() else {
        return Err(PostgresError::Crypto("a sealed access token is empty"));
    };

    if version != VERSION {
        return Err(PostgresError::Crypto(
            "a sealed access token was written by a scheme this build does not know",
        ));
    }

    if rest.len() <= NONCE {
        return Err(PostgresError::Crypto("a sealed access token is truncated"));
    }

    let (nonce, sealed) = rest.split_at(NONCE);

    // Through an array rather than a slice: the length is already known from
    // the split above, and the slice helper is deprecated.
    let nonce: [u8; NONCE] = nonce
        .try_into()
        .map_err(|_| PostgresError::Crypto("a sealed access token has no nonce"))?;

    let cipher = Aes256Gcm::new(&key.0);

    // GCM verifies the tag before it hands anything back, so a wrong key or a
    // changed byte fails here rather than producing rubbish that looks like a
    // token and is refused by Plaid an hour later.
    let plain = cipher
        .decrypt(&nonce.into(), sealed)
        .map_err(|_| {
            PostgresError::Crypto("could not open the access token — wrong key, or the row changed")
        })?;

    String::from_utf8(plain)
        .map_err(|_| PostgresError::Crypto("an opened access token is not text"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key that is only ever a test's. Thirty-two bytes of nothing much.
    fn key() -> DbKey {
        DbKey::parse(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]))
            .expect("32 bytes is a key")
    }

    #[test]
    fn what_was_sealed_comes_back() {
        let sealed = seal(&key(), "access-sandbox-1234").expect("sealing works");

        assert_eq!(
            open(&key(), &sealed).expect("opening works"),
            "access-sandbox-1234",
        );
    }

    /// The nonce is fresh each time, so the same token seals to a different
    /// blob — which is what stops the column saying which two rows are equal.
    #[test]
    fn the_same_token_seals_differently_twice() {
        let once = seal(&key(), "access-sandbox-1234").expect("sealing works");
        let again = seal(&key(), "access-sandbox-1234").expect("sealing works");

        assert_ne!(once, again);
    }

    #[test]
    fn another_key_cannot_open_it() {
        let sealed = seal(&key(), "access-sandbox-1234").expect("sealing works");

        let other = DbKey::parse(&base64::engine::general_purpose::STANDARD.encode([9u8; 32]))
            .expect("32 bytes is a key");

        assert!(open(&other, &sealed).is_err());
    }

    /// GCM authenticates, so a row somebody edited is refused rather than
    /// opened into something else.
    #[test]
    fn a_changed_row_is_refused() {
        let mut sealed = seal(&key(), "access-sandbox-1234").expect("sealing works");

        let last = sealed.len() - 1;
        sealed[last] ^= 1;

        assert!(open(&key(), &sealed).is_err());
    }

    #[test]
    fn a_truncated_blob_is_refused() {
        assert!(open(&key(), &[]).is_err());
        assert!(open(&key(), &[VERSION]).is_err());
        assert!(open(&key(), &[VERSION, 0, 0, 0]).is_err());
    }

    #[test]
    fn a_blob_from_another_scheme_is_refused() {
        let mut sealed = seal(&key(), "access-sandbox-1234").expect("sealing works");
        sealed[0] = VERSION + 1;

        assert!(open(&key(), &sealed).is_err());
    }

    #[test]
    fn a_key_that_is_not_32_bytes_is_refused() {
        let short = base64::engine::general_purpose::STANDARD.encode([1u8; 16]);
        let long = base64::engine::general_purpose::STANDARD.encode([1u8; 64]);

        assert!(DbKey::parse(&short).is_err());
        assert!(DbKey::parse(&long).is_err());
        assert!(DbKey::parse("not base64 at all!").is_err());
    }
}
