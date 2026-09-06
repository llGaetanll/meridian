//! How the `public_token` gets back to us after a Link session.

use src_plaid::PlaidError;

/// Which of the two channels collects the result of a Link session.
///
/// Not `PLAID_ENV`, and must not be folded into it: the choice is about where
/// *we* are running, not which Plaid we are talking to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaidCollect {
    /// Ask `/link/token/get` what became of the session, until it says. Needs
    /// no public URL, which is what makes it the one that works from a laptop.
    Poll,

    /// Wait for Plaid to call `SESSION_FINISHED` on us. Prompter, and requires
    /// an address Plaid can reach.
    Webhook,
}

impl PlaidCollect {
    /// The channel named by `PLAID_COLLECT`. No default: guessing `Webhook` on
    /// a laptop waits forever for a call that cannot arrive.
    pub fn from_env() -> Result<Self, PlaidError> {
        let value = std::env::var("PLAID_COLLECT").map_err(|_| {
            PlaidError::Config(String::from(
                "PLAID_COLLECT is not set — it must be `poll` or `webhook`",
            ))
        })?;

        Self::parse(&value)
    }

    /// The channel a string names, exactly. Separate from
    /// [`from_env`](PlaidCollect::from_env) so the parse is testable without
    /// the process environment, which every test in the binary shares.
    pub fn parse(value: &str) -> Result<Self, PlaidError> {
        match value {
            "poll" => Ok(PlaidCollect::Poll),
            "webhook" => Ok(PlaidCollect::Webhook),
            other => Err(PlaidError::Config(format!(
                "PLAID_COLLECT is `{other}` — it must be `poll` or `webhook`",
            ))),
        }
    }

    /// What the logs call it.
    pub fn as_str(&self) -> &'static str {
        match self {
            PlaidCollect::Poll => "poll",
            PlaidCollect::Webhook => "webhook",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_channels_parse() {
        assert_eq!(PlaidCollect::parse("poll").unwrap(), PlaidCollect::Poll);
        assert_eq!(
            PlaidCollect::parse("webhook").unwrap(),
            PlaidCollect::Webhook,
        );
    }

    #[test]
    fn anything_else_is_an_error() {
        for value in ["", "polling", "Poll", "hook", "dev"] {
            assert!(
                PlaidCollect::parse(value).is_err(),
                "`{value}` must not resolve to a channel",
            );
        }
    }
}
