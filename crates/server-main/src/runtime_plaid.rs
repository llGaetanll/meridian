//! Plaid, as the server's actions reach it.
//!
//! Plain async functions over plain data, called by the runtime and by nothing
//! that decides anything. Everything here needs the Plaid secret, which grants
//! access to every Item on the account and so cannot ship inside an app.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;
use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidEnv;
use src_plaid::PlaidError;

use crate::project;

/// One page of `/transactions/sync`, projected.
///
/// A **diff**, not a snapshot: a second call usually returns three empty
/// lists, and reading that as "the ledger is empty" is the trap.
pub struct Page {
    pub added: Vec<Entry>,
    pub modified: Vec<Entry>,

    /// Ids of transactions the bank retracted.
    pub removed: Vec<String>,

    /// Where to resume, whether or not there is more behind it.
    pub cursor: String,

    /// Whether Plaid has another page after this one.
    pub has_more: bool,
}

/// What every account on one bank holds, and what the bank is called.
///
/// The first place the institution's name exists: it is in neither the Link
/// session nor the exchange.
pub async fn fetch_balances(
    credentials: &PlaidCredentials,
    access_token: &str,
    now: api_common::Timestamp,
) -> Result<Connection, PlaidError> {
    let client = client(credentials)?;

    let balances = src_plaid::fetch_balances(&client, access_token).await?;

    Ok(project::connection(&balances, now))
}

/// Everything that changed on one bank since `cursor`, one page of it.
pub async fn sync_transactions(
    credentials: &PlaidCredentials,
    access_token: &str,
    cursor: Option<String>,
) -> Result<Page, PlaidError> {
    let client = client(credentials)?;

    let sync = src_plaid::sync_transactions(&client, access_token, cursor).await?;

    Ok(Page {
        added: sync.added.iter().map(project::entry).collect(),
        modified: sync.modified.iter().map(project::entry).collect(),
        removed: sync
            .removed
            .iter()
            .map(|r| r.transaction_id.clone())
            .collect(),
        cursor: sync.next_cursor,
        has_more: sync.has_more,
    })
}

/// The charges Plaid has worked out come back on this bank.
///
/// Derived from transactions, so a freshly created Item has none until its
/// history has landed.
pub async fn fetch_recurring(
    credentials: &PlaidCredentials,
    access_token: &str,
) -> Result<Vec<Stream>, PlaidError> {
    let client = client(credentials)?;

    let recurring = src_plaid::fetch_recurring(&client, access_token).await?;

    Ok(project::streams(&recurring))
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

/// A whole app's worth of data, as one value.
///
/// Only the fixtures answer in this shape. A real sync is three calls the
/// machine makes one at a time, and each of them writes what it brought.
pub struct Fetched {
    pub added: Vec<Entry>,
    pub modified: Vec<Entry>,
    pub removed: Vec<String>,
    pub connections: Vec<Connection>,
    pub streams: Vec<Stream>,
    pub cursor: String,
}

/// Everything the app needs, from the captured Sandbox responses.
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
