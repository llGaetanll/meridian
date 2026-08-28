//! Which Plaid to talk to.
//!
//! Said out loud or not at all. There is no default and nothing is inferred:
//! `PLAID_ENV` names the environment or the process does not start.
//!
//! This used to default to Sandbox, on the reasoning that the lazy path should
//! be the harmless one. The reasoning was sound and the mechanism was not — a
//! default is a decision taken by whoever forgot to look, and "unset",
//! "empty", "sandbx" and "leftover from another project" all quietly meant the
//! same safe thing right up until one of them did not. Requiring the variable
//! removes the class: an environment that has not said which Plaid it means
//! cannot reach either one.
//!
//! See `docs/PLAID.md` for what is at stake. The account is capped at ten
//! Production Items and removing one does not give the slot back.

use crate::PlaidError;

/// Fake institutions, `user_good` / `pass_good`, no limits.
pub const SANDBOX_BASE_URL: &str = "https://sandbox.plaid.com/";

/// Real banks and real money. Ten Items, forever.
pub const PRODUCTION_BASE_URL: &str = "https://production.plaid.com/";

/// Deliberately not `Default`. A default environment is the silent fallback
/// this module exists to remove, and deriving one would put it back within
/// reach of anything that builds a struct with `..Default::default()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaidEnv {
    Sandbox,
    Production,
}

impl PlaidEnv {
    /// The environment named by `PLAID_ENV`.
    ///
    /// Required, and matched exactly. A misspelling is an error rather than a
    /// silent fall back to Sandbox: the two failures look identical from the
    /// outside — no data — and only one of them is fixed by checking the
    /// spelling of a variable nobody suspects.
    pub fn from_env() -> Result<Self, PlaidError> {
        let value = std::env::var("PLAID_ENV").map_err(|_| {
            PlaidError::Config(String::from(
                "PLAID_ENV is not set — it must be `sandbox` or `production`",
            ))
        })?;

        Self::parse(&value)
    }

    /// The environment a string names, exactly.
    ///
    /// Separate from [`from_env`](PlaidEnv::from_env) so the parse can be
    /// tested without touching the process environment, which is global and
    /// shared with every other test in the binary.
    pub fn parse(value: &str) -> Result<Self, PlaidError> {
        match value {
            "sandbox" => Ok(PlaidEnv::Sandbox),
            "production" => Ok(PlaidEnv::Production),
            other => Err(PlaidError::Config(format!(
                "PLAID_ENV is `{other}` — it must be `sandbox` or `production`",
            ))),
        }
    }

    pub fn base_url(&self) -> &'static str {
        match self {
            PlaidEnv::Sandbox => SANDBOX_BASE_URL,
            PlaidEnv::Production => PRODUCTION_BASE_URL,
        }
    }

    /// What the logs call it, so which environment a session ran against is
    /// answerable from the log alone.
    pub fn as_str(&self) -> &'static str {
        match self {
            PlaidEnv::Sandbox => "sandbox",
            PlaidEnv::Production => "production",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_environments_parse_to_their_hosts() {
        assert_eq!(
            PlaidEnv::parse("sandbox").unwrap().base_url(),
            SANDBOX_BASE_URL
        );
        assert_eq!(
            PlaidEnv::parse("production").unwrap().base_url(),
            PRODUCTION_BASE_URL,
        );
    }

    /// The whole point of requiring the variable: none of these is Sandbox.
    /// Each one used to be.
    #[test]
    fn anything_else_is_an_error_rather_than_a_quiet_sandbox() {
        for value in ["", "Sandbox", "sandbx", "dev", "SANDBOX", "prod"] {
            assert!(
                PlaidEnv::parse(value).is_err(),
                "`{value}` must not resolve to an environment",
            );
        }
    }
}
