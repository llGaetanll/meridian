//! The bearer token the phone holds, and how a request is read for one.

use sha2::Digest;
use sha2::Sha256;

/// What is stored for a token. Never the token itself.
pub fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// The token on a request, if it carries one.
///
/// `Authorization: Bearer <token>`, and nothing else — not a query parameter,
/// which would land in every access log between here and the phone, and not a
/// cookie, which the app does not use.
pub fn from_header(header: Option<&str>) -> Option<&str> {
    let value = header?;

    let token = value.strip_prefix("Bearer ")?.trim();

    if token.is_empty() { None } else { Some(token) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bearer_token_is_read_off_the_header() {
        assert_eq!(from_header(Some("Bearer abc123")), Some("abc123"));
    }

    /// Everything that is not exactly a bearer token is no token at all,
    /// rather than something to have a guess at.
    #[test]
    fn anything_else_is_nothing() {
        for header in [
            None,
            Some(""),
            Some("abc123"),
            Some("Basic abc123"),
            Some("Bearer "),
        ] {
            assert_eq!(from_header(header), None, "{header:?}");
        }
    }

    #[test]
    fn the_hash_is_not_the_token() {
        assert_ne!(hash("abc123"), b"abc123".to_vec());
        assert_eq!(hash("abc123"), hash("abc123"));
        assert_ne!(hash("abc123"), hash("abc124"));
    }
}
