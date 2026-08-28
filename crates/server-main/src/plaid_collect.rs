//! How the `public_token` gets back to us after a Link session.

use src_plaid::PlaidError;

/// Which of the two channels collects the result of a Link session.
///
/// The completion redirect closes the sheet and carries no token
/// (`docs/PLAID.md`), so the token arrives on one of these instead. They are
/// independent of each other, and both are Plaid's own — the choice is about
/// where *we* are running, not about which Plaid we are talking to.
///
/// That is why this is not `PLAID_ENV` and must not be folded into it. Polling
/// against Production is perfectly sensible, and so is a webhook against
/// Sandbox once there is a tunnel to receive it. Two variables that both read
/// as "which environment" get conflated exactly once, at the worst moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaidCollect {
    /// Ask `/link/token/get` what became of the session, until it says.
    ///
    /// Needs no public URL, which is what makes it the one that works from a
    /// laptop. Plaid holds the answer for six hours after a session finishes,
    /// so a poll that starts when the sheet closes cannot miss it.
    Poll,

    /// Wait for Plaid to call `SESSION_FINISHED` on us.
    ///
    /// Prompter, and what a deployed server should prefer. Requires an address
    /// Plaid can reach, which a machine on a home network does not have.
    Webhook,
}

impl PlaidCollect {
    /// The channel named by `PLAID_COLLECT`.
    ///
    /// Required and matched exactly, like every other variable this server
    /// reads. There is no default because neither answer is safe to assume:
    /// guessing `Poll` on a deployed server spends requests on something a
    /// webhook already delivered, and guessing `Webhook` on a laptop waits
    /// forever for a call that cannot arrive.
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
