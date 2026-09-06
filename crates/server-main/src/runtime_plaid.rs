//! Plaid, as the server's actions reach it.
//!
//! Plain async functions over plain data, called by the runtime and by nothing
//! that decides anything. Everything here needs the Plaid secret, which grants
//! access to every Item on the account and so cannot ship inside an app.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;
use server_wire::SyncStage;
use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidEnv;
use src_plaid::PlaidError;

use crate::project;

/// What one trip to Plaid turned up.
///
/// A **diff**, not a snapshot: a second call usually returns three empty
/// lists, and reading that as "the ledger is empty" is the trap.
pub struct Fetched {
    pub added: Vec<Entry>,
    pub modified: Vec<Entry>,

    /// Ids of transactions the bank retracted.
    pub removed: Vec<String>,

    pub connections: Vec<Connection>,
    pub streams: Vec<Stream>,
    pub cursor: String,
}

/// Everything the app needs, in as few calls as Plaid allows.
///
/// **Sequential, and deliberately so.** The three endpoints could run at once
/// and halve a first sync, but the first-run screen is three rows filling in
/// one at a time and has nothing to draw if they land together. `report` is
/// called as each stage completes, with what is known so far.
pub async fn fetch_all(
    credentials: &PlaidCredentials,
    access_token: &str,
    cursor: Option<String>,
    now: api_common::Timestamp,
    report: impl Fn(SyncStage, Option<&str>, u32, u32, u32),
) -> Result<Fetched, PlaidError> {
    let client = client(credentials)?;

    let token = access_token;

    // Not even the bank's name is known yet: it is in neither the Link session
    // nor the exchange, and the balances call below is where it appears.
    report(SyncStage::Accounts, None, 0, 0, 0);

    let balances = src_plaid::fetch_balances(&client, token).await?;
    let connections = vec![project::connection(&balances, now)];

    let accounts = connections
        .first()
        .map_or(0, |c| c.accounts.len().try_into().unwrap_or(u32::MAX));

    let institution = connections.first().map(|c| c.name.as_str());

    report(SyncStage::Transactions, institution, accounts, 0, 0);

    let sync = src_plaid::sync_transactions(&client, token, cursor).await?;

    let transactions = sync.added.len().try_into().unwrap_or(u32::MAX);

    report(SyncStage::Streams, institution, accounts, transactions, 0);

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

    // No fourth report: nothing could observe it, because the fetch answers in
    // the same turn. The stream count arrives with the snapshot instead.

    Ok(Fetched {
        added: sync.added.iter().map(project::entry).collect(),
        modified: sync.modified.iter().map(project::entry).collect(),
        removed: sync.removed,
        connections,
        streams,
        cursor: sync.cursor,
    })
}

/// Open a Link session, and answer with the token and the page that hosts it.
///
/// `client_user_id` is our own id for the person linking. A uuid rather than
/// an address: it ends up in Plaid's logs.
///
/// `webhook` is where Plaid should call when the session ends; `None` leaves
/// polling to collect the result.
pub async fn open_link(
    credentials: &PlaidCredentials,
    client_user_id: &str,
    webhook: Option<&str>,
) -> Result<(String, Option<String>), PlaidError> {
    let client = client(credentials)?;

    // `is_mobile_app` is false because true needs a top-level `redirect_uri`
    // that is an https address registered in the dashboard. It costs nothing
    // in Sandbox, which has no OAuth banks, and has to be revisited before a
    // real bank is linked from a real phone.
    let request = src_plaid::LinkTokenRequest::hosted(client_user_id)
        .redirecting_to(COMPLETION_REDIRECT, false);

    let request = match webhook {
        Some(webhook) => request.notifying(webhook),
        None => request,
    };

    let res = src_plaid::create_link_token(&client, &request).await?;

    Ok((res.link_token, res.hosted_link_url))
}

/// Where Plaid sends the browser when the flow ends. It carries no token; the
/// `public_token` arrives by webhook or by polling.
const COMPLETION_REDIRECT: &str = "meridian://linked";

/// What became of a Link session: whether it has ended, and what it produced.
pub async fn poll_link(
    credentials: &PlaidCredentials,
    link_token: &str,
) -> Result<(bool, Vec<String>), PlaidError> {
    let client = client(credentials)?;

    let res = src_plaid::link_session(&client, link_token).await?;

    Ok((
        res.finished(),
        res.public_tokens().iter().map(|t| t.to_string()).collect(),
    ))
}

/// The public key one webhook was signed with.
///
/// The only function here whose caller is an unauthenticated route. Keeping
/// that from becoming a way to make this server call Plaid at will is on the
/// caller, which checks the algorithm first and caches the answer by `kid`.
pub async fn webhook_key(
    credentials: &PlaidCredentials,
    key_id: &str,
) -> Result<src_plaid::PlaidJwk, PlaidError> {
    let client = client(credentials)?;

    src_plaid::fetch_webhook_key(&client, key_id).await
}

/// Trade a `public_token` for the permanent one, and the Item it belongs to.
pub async fn exchange(
    credentials: &PlaidCredentials,
    public_token: &str,
) -> Result<(String, String), PlaidError> {
    let client = client(credentials)?;

    let res = src_plaid::exchange_public_token(&client, public_token).await?;

    Ok((res.item_id, res.access_token))
}

/// A client for these credentials, refusing anything but Sandbox.
///
/// Every call to Plaid goes through here, so nothing can reach Production by
/// accident.
fn client(credentials: &PlaidCredentials) -> Result<PlaidClient, PlaidError> {
    if credentials.env != PlaidEnv::Sandbox {
        tracing::error!(
            env = credentials.env.as_str(),
            "refusing to call anything but the Plaid Sandbox — see docs/PLAID.md",
        );

        return Err(PlaidError::InvalidUrl(
            "server-main only ever talks to the Plaid Sandbox".to_string(),
        ));
    }

    PlaidClient::new(PlaidCredentials {
        client_id: credentials.client_id.clone(),
        secret: credentials.secret.clone(),
        env: credentials.env,
    })
}

/// The same thing, from the captured Sandbox responses.
///
/// For `examples/snapshot.rs`, which generates the corpus `phone-state`'s
/// tests read and wants a fixed answer. Nothing the server serves comes
/// through here: it starts with credentials or it does not start.
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
