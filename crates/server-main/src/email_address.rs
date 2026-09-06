//! What this server will accept as somewhere to send a sign-in code.
//!
//! Deliberately shallow: the only way to learn that an address accepts mail is
//! to send to it, so all this rules out is the string that could never be one.
//! It lives behind the machine because it decides whether mail is sent.

/// Whether this could be an address at all.
///
/// Exactly one `@`, with something on each side of it.
pub(crate) fn plausible(email: &str) -> bool {
    let mut parts = email.split('@');

    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };

    !local.is_empty() && !domain.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_has_something_on_both_sides_of_one_at() {
        assert!(plausible("alex@example.com"));
        assert!(plausible("a@b"));
    }

    #[test]
    fn everything_that_could_never_be_delivered_to_is_refused() {
        assert!(!plausible(""), "empty");
        assert!(!plausible("alex"), "no at");
        assert!(!plausible("@example.com"), "nobody to send to");
        assert!(!plausible("alex@"), "nowhere to send it");
        assert!(!plausible("alex@@example.com"), "two ats");
    }
}
