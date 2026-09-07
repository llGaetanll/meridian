//! What this server will accept as somewhere to send a sign-in code, and who
//! it will send one to at all.
//!
//! [`plausible`] is deliberately shallow: the only way to learn that an
//! address accepts mail is to send to it, so all it rules out is the string
//! that could never be one. [`allowed`] is the opposite — it names one
//! address and refuses the rest of the world.
//!
//! Both live behind the machine because both decide whether mail is sent.

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

/// The one address that may have an account here.
///
/// Not a security measure, and it protects nothing an attacker wants: it
/// rations the ten Plaid Item slots the Trial plan allows. Every account that
/// links a bank spends one, `/item/remove` does not give it back, and there is
/// no eleventh to buy — so ten strangers doing exactly what the app invites
/// them to do is an account with no room left for the people it was built for.
///
/// Compiled in rather than configured, because it is temporary and small. It
/// comes off when the Plaid plan stops rationing Items and Stripe is what
/// decides who is a customer; at that point signing up is the thing we want
/// rather than the thing that costs us. Issue #76.
const ALLOWED: &str = "gaetan@example.com";

/// Whether this address may sign up.
///
/// Trimmed and compared without case, and exactly otherwise — a `+tag` is one
/// of the things being refused. `identities` keys on the address as written
/// (`0005_identities_and_codes.sql`), so a tag is a second account out of one
/// inbox, which is the whole shape this is here to stop.
pub(crate) fn allowed(email: &str) -> bool {
    email.trim().eq_ignore_ascii_case(ALLOWED)
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

    #[test]
    fn one_address_may_sign_up_and_nobody_else_may() {
        assert!(allowed("gaetan@example.com"));
        assert!(allowed("  gaetan@example.com  "), "trimmed");
        assert!(
            allowed("Gaetan.Almela@Gmail.com"),
            "a capital is the same address"
        );
    }

    /// Each of these reaches the same inbox as the address above, and each
    /// would be a second account keyed on the string as written.
    #[test]
    fn a_tag_and_a_stranger_are_both_refused() {
        assert!(
            !allowed("gaetan.almela+1@gmail.com"),
            "a tag is a second account"
        );
        assert!(
            !allowed("gaetanalmela@gmail.com"),
            "gmail ignores the dot, we do not"
        );
        assert!(!allowed("gaetan.almela@example.com"), "another domain");
        assert!(!allowed("someone@example.com"), "somebody else entirely");
        assert!(!allowed(""), "nothing at all");
    }
}
