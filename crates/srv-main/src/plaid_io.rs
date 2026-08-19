//! The IO the server's actions ask for.
//!
//! Plain async functions over plain data, called by the runtime and by nothing
//! that decides anything. Everything here needs the Plaid secret, which is the
//! reason this crate exists at all: `docs/PLAID.md` is explicit that the
//! secret grants access to every Item on the account and so cannot ship inside
//! an app anyone can unzip.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Ledger;
use api_ledger::Stream;
use src_plaid::PEnv;
use src_plaid::PError;
use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;

use crate::project;

/// What one trip to Plaid turned up.
pub struct Fetched {
    pub entries: Vec<Entry>,
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
    now: u64,
) -> Result<Fetched, PError> {
    // Guard rail, not decoration. Nothing should be able to reach Production
    // by accident, and a panic here is far cheaper than a permanent Item.
    assert_eq!(
        config.credentials.env,
        PEnv::Sandbox,
        "srv-main only ever talks to the Plaid Sandbox — see docs/PLAID.md",
    );

    let client = PlaidClient::new(PlaidCredentials {
        client_id: config.credentials.client_id.clone(),
        secret: config.credentials.secret.clone(),
        env: config.credentials.env,
    })?;

    let token = &config.access_token;

    let sync = src_plaid::sync_transactions(&client, token, cursor).await?;

    let mut ledger = Ledger::default();
    ledger.apply(
        sync.added.iter().map(project::entry).collect(),
        sync.modified.iter().map(project::entry).collect(),
        &sync.removed,
    );

    let balances = src_plaid::fetch_balances(&client, token).await?;
    let connections = vec![project::connection(&balances, now)];

    // Recurring is derived from transactions, so a freshly created Item has
    // none yet. That is a thin screen, not a failed sync.
    let streams = match src_plaid::fetch_recurring(&client, token).await {
        Ok(recurring) => project::streams(&recurring),
        Err(e) => {
            tracing::warn!(
                target: "app",
                error = %e,
                "recurring unavailable, continuing without it",
            );
            Vec::new()
        }
    };

    Ok(Fetched {
        entries: ledger.entries().to_vec(),
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
pub fn fetch_fixtures(now: u64) -> Fetched {
    let sync: src_plaid::SyncResponse =
        serde_json::from_str(include_str!("../fixtures/sync.json")).expect("fixture parses");
    let balances: src_plaid::BalancesResponse =
        serde_json::from_str(include_str!("../fixtures/balances.json")).expect("fixture parses");
    let recurring: src_plaid::RecurringResponse =
        serde_json::from_str(include_str!("../fixtures/recurring.json")).expect("fixture parses");

    let mut ledger = Ledger::default();
    ledger.apply(
        sync.added.iter().map(project::entry).collect(),
        sync.modified.iter().map(project::entry).collect(),
        &sync
            .removed
            .iter()
            .map(|r| r.transaction_id.clone())
            .collect::<Vec<_>>(),
    );

    Fetched {
        entries: ledger.entries().to_vec(),
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
        let fetched = fetch_fixtures(1_787_097_600);

        assert_eq!(fetched.entries.len(), 48);
        assert_eq!(fetched.connections.len(), 1);
        assert_eq!(fetched.connections[0].accounts.len(), 14);
        assert_eq!(fetched.streams.len(), 8);
        assert!(!fetched.cursor.is_empty());
    }
}
