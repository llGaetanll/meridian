//! Plaid, as the server's actions reach it.
//!
//! Plain async functions over plain data, called by the runtime and by nothing
//! that decides anything. Everything here needs the Plaid secret, which is the
//! reason this crate exists at all: `docs/PLAID.md` is explicit that the
//! secret grants access to every Item on the account and so cannot ship inside
//! an app anyone can unzip.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;
use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidEnv;
use src_plaid::PlaidError;

use crate::project;

/// What one trip to Plaid turned up.
///
/// A **diff**, not a snapshot. `/transactions/sync` answers with what has
/// changed since the cursor, so a second call usually returns three empty
/// lists — and treating that as "the ledger is now empty" is the whole trap.
/// The caller applies this to what it already holds.
pub struct Fetched {
    pub added: Vec<Entry>,
    pub modified: Vec<Entry>,

    /// Ids of transactions the bank retracted.
    pub removed: Vec<String>,

    pub connections: Vec<Connection>,
    pub streams: Vec<Stream>,
    pub cursor: String,
}

/// The credentials and token a live fetch needs.
pub struct PlaidConfig {
    pub credentials: PlaidCredentials,
    pub access_token: String,
}

impl PlaidConfig {
    /// Read the environment, or report why a live fetch is not possible.
    ///
    /// Missing credentials are not an error: the server falls back to the
    /// fixtures, so a clean checkout with no `.env` still runs the app.
    pub fn from_env() -> Option<Self> {
        let access_token = std::env::var("PLAID_ACCESS_TOKEN").ok()?;

        // `PlaidCredentials::from_env` reads PLAID_ENV, which defaults to
        // Sandbox and needs the literal string "production" to be anything
        // else. Nothing in this crate ever sets it.
        let credentials = PlaidCredentials::from_env().ok()?;

        if access_token.trim().is_empty() {
            return None;
        }

        Some(Self {
            credentials,
            access_token,
        })
    }
}

/// Everything the app needs, in as few calls as Plaid allows.
///
/// The three endpoints are independent, so a failure in the optional two does
/// not sink the whole fetch: transactions are what the app is for, and a
/// missing recurring list costs one screen rather than all of them.
pub async fn fetch_all(
    config: &PlaidConfig,
    cursor: Option<String>,
    now: api_common::Timestamp,
) -> Result<Fetched, PlaidError> {
    // Guard rail, not decoration: nothing should be able to reach Production
    // by accident. An error rather than a panic — `docs/STYLE.md` — but the
    // effect is the same, because the caller has no way to proceed past it.
    if config.credentials.env != PlaidEnv::Sandbox {
        tracing::error!(
            env = config.credentials.env.as_str(),
            "refusing to call anything but the Plaid Sandbox — see docs/PLAID.md",
        );

        return Err(PlaidError::InvalidUrl(
            "server-main only ever talks to the Plaid Sandbox".to_string(),
        ));
    }

    let client = PlaidClient::new(PlaidCredentials {
        client_id: config.credentials.client_id.clone(),
        secret: config.credentials.secret.clone(),
        env: config.credentials.env,
    })?;

    let token = &config.access_token;

    let sync = src_plaid::sync_transactions(&client, token, cursor).await?;

    let balances = src_plaid::fetch_balances(&client, token).await?;
    let connections = vec![project::connection(&balances, now)];

    // Recurring is derived from transactions, so a freshly created Item has
    // none yet. That is a thin screen, not a failed sync.
    let streams = match src_plaid::fetch_recurring(&client, token).await {
        Ok(recurring) => project::streams(&recurring),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "recurring unavailable, continuing without it",
            );
            Vec::new()
        }
    };

    Ok(Fetched {
        added: sync.added.iter().map(project::entry).collect(),
        modified: sync.modified.iter().map(project::entry).collect(),
        removed: sync.removed,
        connections,
        streams,
        cursor: sync.cursor,
    })
}

/// The same thing, from the captured Sandbox responses.
///
/// This is what makes the app runnable with no credentials at all — and it is
/// the same code path afterwards, so a screen that works on fixtures works on
/// live data.
///
/// The fixtures are compiled in, so a parse failure is a build-time mistake
/// rather than anything a user can cause. It still does not panic: an empty
/// answer and a loud log beat taking the server down, and a test covers the
/// happy path so a broken fixture cannot reach a release unnoticed.
pub fn fetch_fixtures(now: api_common::Timestamp) -> Fetched {
    fn parse<T: serde::de::DeserializeOwned + Default>(name: &str, json: &str) -> T {
        match serde_json::from_str(json) {
            Ok(value) => value,
            Err(e) => {
                tracing::error!(
                    fixture = name,
                    error = %e,
                    "a compiled-in fixture failed to parse",
                );

                T::default()
            }
        }
    }

    let sync: src_plaid::SyncResponse = parse("sync", include_str!("../fixtures/sync.json"));
    let balances: src_plaid::BalancesResponse =
        parse("balances", include_str!("../fixtures/balances.json"));
    let recurring: src_plaid::RecurringResponse =
        parse("recurring", include_str!("../fixtures/recurring.json"));

    Fetched {
        added: sync.added.iter().map(project::entry).collect(),
        modified: sync.modified.iter().map(project::entry).collect(),
        removed: sync
            .removed
            .iter()
            .map(|r| r.transaction_id.clone())
            .collect(),
        connections: vec![project::connection(&balances, now)],
        streams: project::streams(&recurring),
        cursor: sync.next_cursor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback has to produce a whole app's worth of data, or the
    /// no-credentials path is not actually usable.
    #[test]
    fn the_fixtures_produce_every_screens_data() {
        let fetched = fetch_fixtures(api_common::Timestamp::from_secs(1_787_097_600));

        assert_eq!(fetched.added.len(), 48);
        assert_eq!(fetched.connections.len(), 1);
        assert_eq!(fetched.connections[0].accounts.len(), 14);
        assert_eq!(fetched.streams.len(), 8);
        assert!(!fetched.cursor.is_empty());
    }
}
