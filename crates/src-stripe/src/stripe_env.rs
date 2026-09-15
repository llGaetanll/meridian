//! Which Stripe to talk to.
//!
//! Said out loud or not at all, the way `PLAID_ENV` is. There is no default
//! and nothing is inferred from the key: `STRIPE_ENV` names the mode or the
//! process does not start, and a key whose prefix says otherwise is refused.
//!
//! Stripe has one host for both modes, and the key alone picks which one a
//! call lands in. That is what makes the variable worth having: a live key
//! works perfectly, charges a real card, and looks exactly like a test key
//! from every line of code that uses it. Naming the mode separately means a
//! key that reached the wrong `.env` fails on the way up instead of working.

use crate::StripeError;

/// Deliberately not `Default`. A default mode is the silent fallback this
/// module exists to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripeEnv {
    /// `sk_test_` and `pk_test_`. Test cards, no money.
    Test,

    /// `sk_live_` and `pk_live_`. Real cards and real charges.
    Live,
}

impl StripeEnv {
    /// The mode named by `STRIPE_ENV`.
    ///
    /// Required, and matched exactly. A misspelling is an error rather than a
    /// fall back to test.
    pub fn from_env() -> Result<Self, StripeError> {
        let value = std::env::var("STRIPE_ENV").map_err(|_| {
            StripeError::Config(String::from(
                "STRIPE_ENV is not set — it must be `test` or `live`",
            ))
        })?;

        Self::parse(&value)
    }

    /// The mode a string names, exactly.
    ///
    /// Separate from [`from_env`](StripeEnv::from_env) so the parse can be
    /// tested without touching the process environment.
    pub fn parse(value: &str) -> Result<Self, StripeError> {
        match value {
            "test" => Ok(StripeEnv::Test),
            "live" => Ok(StripeEnv::Live),
            other => Err(StripeError::Config(format!(
                "STRIPE_ENV is `{other}` — it must be `test` or `live`",
            ))),
        }
    }

    /// The mode a key belongs to, read off its prefix. `None` is a string
    /// that is not a Stripe key at all.
    ///
    /// Secret and publishable keys carry the same prefix after the first two
    /// letters, so one reading serves both.
    pub fn of_key(key: &str) -> Option<Self> {
        let mode = key
            .strip_prefix("sk_")
            .or_else(|| key.strip_prefix("pk_"))?;

        if mode.starts_with("test_") {
            Some(StripeEnv::Test)
        } else if mode.starts_with("live_") {
            Some(StripeEnv::Live)
        } else {
            None
        }
    }

    /// What the logs call it, so which mode a session ran in is answerable
    /// from the log alone.
    pub fn as_str(&self) -> &'static str {
        match self {
            StripeEnv::Test => "test",
            StripeEnv::Live => "live",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_modes_parse() {
        assert_eq!(StripeEnv::parse("test").unwrap(), StripeEnv::Test);
        assert_eq!(StripeEnv::parse("live").unwrap(), StripeEnv::Live);
    }

    /// The whole point of requiring the variable: none of these is test.
    #[test]
    fn anything_else_is_an_error_rather_than_a_quiet_test() {
        for value in ["", "Test", "sandbox", "production", "LIVE", "prod"] {
            assert!(
                StripeEnv::parse(value).is_err(),
                "`{value}` must not resolve to a mode",
            );
        }
    }

    /// Both kinds of key say which mode they are, and a string that is
    /// neither says nothing.
    #[test]
    fn a_key_names_its_mode() {
        assert_eq!(StripeEnv::of_key("sk_test_abc"), Some(StripeEnv::Test));
        assert_eq!(StripeEnv::of_key("pk_test_abc"), Some(StripeEnv::Test));
        assert_eq!(StripeEnv::of_key("sk_live_abc"), Some(StripeEnv::Live));
        assert_eq!(StripeEnv::of_key("pk_live_abc"), Some(StripeEnv::Live));

        assert_eq!(StripeEnv::of_key(""), None);
        assert_eq!(StripeEnv::of_key("rk_live_abc"), None);
        assert_eq!(StripeEnv::of_key("sk_abc"), None);
        assert_eq!(StripeEnv::of_key("whsec_abc"), None);
    }
}
