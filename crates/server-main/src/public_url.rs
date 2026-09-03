//! Where this server can be reached from outside.
//!
//! The one thing a process cannot work out about itself. It knows the port it
//! bound and nothing about the name, the scheme or the proxy in front of it,
//! so an address it is meant to hand to somebody else has to be told to it.

use src_plaid::PlaidError;

/// The route Plaid is told to call, mounted by
/// [`router_server`](crate::router_server).
///
/// Here rather than beside the route so that the address we advertise and the
/// address we answer on cannot drift apart — they are the same constant, used
/// twice.
pub const WEBHOOK_PATH: &str = "/plaid/webhook";

/// This server's address on the internet, without a trailing slash.
///
/// Required only when `PLAID_COLLECT=webhook`, the same way `RESEND_API_KEY`
/// is required only when `EMAIL_SEND=resend`: a variable that has no meaning
/// in the mode being run is not one to insist on. In the mode where it *does*
/// have meaning it is required absolutely, because the alternative — guessing
/// at localhost, or sending Plaid no webhook and running on quietly — is a
/// server that looks configured for webhooks and collects nothing.
#[derive(Debug, Clone)]
pub struct PublicUrl(String);

impl PublicUrl {
    /// The address named by `PUBLIC_URL`.
    pub fn from_env() -> Result<Self, PlaidError> {
        let value = std::env::var("PUBLIC_URL").map_err(|_| {
            PlaidError::Config(String::from(
                "PUBLIC_URL is not set — PLAID_COLLECT=webhook needs an address Plaid can reach",
            ))
        })?;

        Self::parse(&value)
    }

    /// The address a string names, checked on the way up.
    ///
    /// Separate from [`from_env`](PublicUrl::from_env) so the parse is
    /// testable without the process environment, which every test in the
    /// binary shares.
    ///
    /// Checked by hand rather than by a URL parser, and only for the four
    /// things that make an address unusable *here*: Plaid refuses to call
    /// anything but https, a host is what the address is for, and a query or
    /// a fragment would be silently dropped by the path this is joined with.
    /// A parser would accept all four and leave the mistake to be found on the
    /// day somebody links a bank.
    pub fn parse(value: &str) -> Result<Self, PlaidError> {
        let value = value.trim().trim_end_matches('/');

        let refuse = |why: &str| Err(PlaidError::Config(format!("PUBLIC_URL {why}: `{value}`")));

        let Some(host) = value.strip_prefix("https://") else {
            return refuse("must start with https:// — Plaid will not call anything else");
        };

        if host.is_empty() || host.starts_with('/') {
            return refuse("names no host");
        }

        if host.contains(char::is_whitespace) {
            return refuse("has whitespace in it");
        }

        if value.contains('?') || value.contains('#') {
            return refuse("must be a bare address, with no query or fragment");
        }

        Ok(Self(value.to_string()))
    }

    /// Where to tell Plaid to send webhooks.
    pub fn webhook(&self) -> String {
        format!("{}{WEBHOOK_PATH}", self.0)
    }

    /// What the logs call it. It is not a secret — it is the address anybody
    /// on the internet reaches this server at.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_becomes_the_route_plaid_calls() {
        let url = PublicUrl::parse("https://api.meridiancash.io").unwrap();

        assert_eq!(url.webhook(), "https://api.meridiancash.io/plaid/webhook");
    }

    /// A trailing slash is what somebody copying an address out of a browser
    /// ends up with, and joining on it twice gives Plaid a URL nothing answers.
    #[test]
    fn a_trailing_slash_does_not_double_up() {
        let url = PublicUrl::parse("https://api.meridiancash.io/").unwrap();

        assert_eq!(url.webhook(), "https://api.meridiancash.io/plaid/webhook");
    }

    /// Each of these is a server that would start, tell Plaid an address it
    /// cannot use, and wait forever for a webhook.
    #[test]
    fn anything_plaid_cannot_call_is_refused() {
        for value in [
            "",
            "api.meridiancash.io",
            "http://api.meridiancash.io",
            "https://",
            "https:///plaid",
            "https://api.meridiancash.io?x=1",
            "https://api meridiancash.io",
        ] {
            assert!(
                PublicUrl::parse(value).is_err(),
                "`{value}` must not be accepted as an address",
            );
        }
    }
}
