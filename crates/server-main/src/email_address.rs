//! What this server will accept as somewhere to send a sign-in code.
//!
//! Deliberately shallow. An address that works is one that accepts mail, and
//! the only way to learn that is to send to it — so anything cleverer here
//! would be a second and wronger definition of a fact the send already
//! settles. What this rules out is the string that could never be one.
//!
//! It lives behind the machine because it is a *rule*: it decides whether mail
//! is sent. It used to sit in the handler and answer 204 before any event
//! existed, which made it unreachable from `eval!` and invisible in a log.

/// Whether this could be an address at all.
///
/// Exactly one `@`, with something on each side of it. Slightly tighter than
/// the check it replaces, which took `"ab@"` — but the same rule, not a new
/// policy: what is refused is still only what could never be delivered to.
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
