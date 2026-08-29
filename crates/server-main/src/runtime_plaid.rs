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

/// Everything the app needs, in as few calls as Plaid allows.
///
/// The three endpoints are independent, so a failure in the optional two does
/// not sink the whole fetch: transactions are what the app is for, and a
/// missing recurring list costs one screen rather than all of them.
pub async fn fetch_all(
    credentials: &PlaidCredentials,
    access_token: &str,
    cursor: Option<String>,
    now: api_common::Timestamp,
) -> Result<Fetched, PlaidError> {
    // Guard rail, not decoration: nothing should be able to reach Production
    // by accident. `client` refuses anything but Sandbox — an error rather
    // than a panic, per `docs/STYLE.md`, but the effect is the same because
    // the caller has no way to proceed past it.
    let client = client(credentials)?;

    let token = access_token;

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

/// Open a Link session, and answer with the token and the page that hosts it.
///
/// `client_user_id` is our own id for the person linking, which Plaid echoes
/// back in webhooks. A uuid rather than an address: it ends up in Plaid's logs.
pub async fn open_link(
    credentials: &PlaidCredentials,
    client_user_id: &str,
) -> Result<(String, Option<String>), PlaidError> {
    let client = client(credentials)?;

    // The redirect closes the sheet and carries no token — the result is
    // collected afterwards, by polling. See `docs/PLAID.md`.
    //
    // `is_mobile_app` is false, and that is a constraint rather than a choice:
    // Plaid rejects the request unless a top-level `redirect_uri` is set too,
    // and that one has to be an https address registered in the dashboard.
    // False costs nothing here — it changes how OAuth banks hand control back,
    // and Sandbox has none. It has to be revisited before a real bank is
    // linked from a real phone.
    let request = src_plaid::LinkTokenRequest::hosted(client_user_id)
        .redirecting_to(COMPLETION_REDIRECT, false);

    let res = src_plaid::create_link_token(&client, &request).await?;

    Ok((res.link_token, res.hosted_link_url))
}

/// Where Plaid sends the browser when the flow ends.
///
/// A custom scheme whose only job is to close the web session. It carries no
/// token; the `public_token` arrives by polling, which is a separate channel.
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
/// The guard `fetch_all` carries, in the one place every call now goes through.
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
/// **Nothing the server serves comes through here any more.** The only caller
/// is `examples/snapshot.rs`, which generates the corpus `phone-state`'s tests
/// read — a real job, and one that wants a fixed answer rather than a live
/// one. The server itself has no fallback: it starts with credentials or it
/// does not start.
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
