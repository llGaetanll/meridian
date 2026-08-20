//! Which Plaid to talk to.
//!
//! The default is Sandbox, and reaching Production takes saying so out loud.
//! See `docs/PLAID.md` for why that asymmetry is deliberate: the account is
//! capped at ten Production Items and removing one does not give the slot
//! back, so the environment you get by not thinking about it had better be the
//! harmless one.

/// Fake institutions, `user_good` / `pass_good`, no limits.
pub const SANDBOX_BASE_URL: &str = "https://sandbox.plaid.com/";

/// Real banks and real money. Ten Items, forever.
pub const PRODUCTION_BASE_URL: &str = "https://production.plaid.com/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaidEnv {
    #[default]
    Sandbox,
    Production,
}

impl PlaidEnv {
    /// The environment named by `PLAID_ENV`.
    ///
    /// Anything other than exactly `production` is Sandbox — unset, empty,
    /// misspelled, or a leftover value from something else. A typo that
    /// silently reached real data would be the one failure mode worth ruling
    /// out entirely.
    pub fn from_env() -> Self {
        match std::env::var("PLAID_ENV").as_deref() {
            Ok("production") => PlaidEnv::Production,
            _ => PlaidEnv::Sandbox,
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
    fn the_default_is_sandbox() {
        assert_eq!(PlaidEnv::default(), PlaidEnv::Sandbox);
        assert_eq!(PlaidEnv::default().base_url(), SANDBOX_BASE_URL);
    }
}
