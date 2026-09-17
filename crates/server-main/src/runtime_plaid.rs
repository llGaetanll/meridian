//! Plaid, as the server's actions reach it.
//!
//! Plain async functions over plain data, called by the runtime and by nothing
//! that decides anything. Everything here needs the Plaid secret, which grants
//! access to every Item on the account and so cannot ship inside an app.
//!
//! One call each, on the one [`client`] the process holds.

use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;
use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidError;

use crate::OauthRedirect;
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
    client: &PlaidClient,
    access_token: &str,
    now: api_common::Timestamp,
) -> Result<Connection, PlaidError> {
    let accounts = src_plaid::fetch_accounts(client, access_token).await?;

    Ok(project::connection(&accounts, now))
}

/// Everything that changed on one bank since `cursor`, one page of it.
pub async fn sync_transactions(
    client: &PlaidClient,
    access_token: &str,
    cursor: Option<String>,
) -> Result<Page, PlaidError> {
    let sync = src_plaid::sync_transactions(client, access_token, cursor).await?;

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
    client: &PlaidClient,
    access_token: &str,
) -> Result<Vec<Stream>, PlaidError> {
    let recurring = src_plaid::fetch_recurring(client, access_token).await?;

    Ok(project::streams(&recurring))
}

/// Open a Link session, and answer with the token and the page that hosts it.
///
/// `client_user_id` is our own id for the person linking. A uuid rather than
/// an address: it ends up in Plaid's logs.
///
/// `completion_redirect` is where Plaid sends the browser when the flow ends.
/// It carries no token; the `public_token` arrives by webhook or by polling.
///
/// `webhook` is where Plaid should call when the session ends; `None` leaves
/// polling to collect the result.
///
/// `handback` is where an OAuth bank's own app returns the user to ours, and
/// `None` is a session the web app opened, or a deployment whose app cannot
/// yet be reached that way. It is the difference between a bank that hands off
/// to its app and one that keeps the sign-in inside the web session, so it is
/// not a detail that can be defaulted.
pub async fn open_link(
    client: &PlaidClient,
    client_user_id: &str,
    completion_redirect: &str,
    webhook: Option<&str>,
    handback: Option<&OauthRedirect>,
) -> Result<(String, Option<String>), PlaidError> {
    let request =
        src_plaid::LinkTokenRequest::hosted(client_user_id).redirecting_to(completion_redirect);

    let request = match handback {
        Some(handback) => request.handing_back_to(handback.as_str()),
        None => request,
    };

    let request = match webhook {
        Some(webhook) => request.notifying(webhook),
        None => request,
    };

    let res = src_plaid::create_link_token(client, &request).await?;

    Ok((res.link_token, res.hosted_link_url))
}

/// Open an update-mode Link session — repair an Item whose login has expired
/// — and answer with the token and the page that hosts it.
///
/// The same call as [`open_link`] with one difference, and it is the one that
/// matters: the request carries the Item's access token and names no
/// products, so nothing this session can do creates an Item or spends a slot.
/// The token stays the same afterwards; what changes is that the bank starts
/// answering for it again.
pub async fn open_repair(
    client: &PlaidClient,
    client_user_id: &str,
    access_token: &str,
    completion_redirect: &str,
    webhook: Option<&str>,
    handback: Option<&OauthRedirect>,
) -> Result<(String, Option<String>), PlaidError> {
    let request = src_plaid::LinkTokenRequest::update(client_user_id, access_token)
        .redirecting_to(completion_redirect);

    let request = match handback {
        Some(handback) => request.handing_back_to(handback.as_str()),
        None => request,
    };

    let request = match webhook {
        Some(webhook) => request.notifying(webhook),
        None => request,
    };

    let res = src_plaid::create_link_token(client, &request).await?;

    Ok((res.link_token, res.hosted_link_url))
}

/// What became of a Link session: whether it has ended, and what it produced.
pub async fn poll_link(
    client: &PlaidClient,
    link_token: &str,
) -> Result<(bool, Vec<String>), PlaidError> {
    let res = src_plaid::link_session(client, link_token).await?;

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
    client: &PlaidClient,
    key_id: &str,
) -> Result<src_plaid::PlaidJwk, PlaidError> {
    src_plaid::fetch_webhook_key(client, key_id).await
}

/// Trade a `public_token` for the permanent one, and the Item it belongs to.
pub async fn exchange(
    client: &PlaidClient,
    public_token: &str,
) -> Result<(String, String), PlaidError> {
    let res = src_plaid::exchange_public_token(client, public_token).await?;

    Ok((res.item_id, res.access_token))
}

/// Remove one bank at Plaid, which is the only way its bill stops.
///
/// Answers nothing: there is nothing to say about an Item that no longer
/// exists, and whether it went is the `Result`.
pub async fn remove_item(client: &PlaidClient, access_token: &str) -> Result<(), PlaidError> {
    src_plaid::remove_item(client, access_token).await?;

    Ok(())
}

/// The one client every call in this module is made on.
///
/// Built once and shared, so a sync's calls reuse a connection rather than
/// shaking hands three times, and the environment is decided in one place —
/// `PLAID_ENV`, read once on the way up and never again.
///
/// This used to refuse anything but Sandbox, and that one `if` was the whole
/// protection for the account's ten Production Item slots. What replaced it is
/// the three things it was standing in for: who may sign up at all
/// (`email_address::allowed`), a budget on every call that can spend a slot
/// (`PlaidEndpoint`), and a write that is retried rather than dropped once one
/// has been spent ([`Attaching`](crate::Attaching)). A refusal here
/// could only ever say no to everything; those say no to the thing that costs.
pub fn client(credentials: PlaidCredentials) -> Result<PlaidClient, PlaidError> {
    PlaidClient::new(credentials)
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
/// For `examples/snapshot.rs`, which generates the corpus `client-state`'s
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
    let balances: src_plaid::AccountsResponse =
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
