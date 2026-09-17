//! Plaid, the bank connection.
//!
//! Like `src-yahoo` and `src-coingecko` this is IO only: plain async functions
//! over plain data, no signals and no decisions. Requests carry a path
//! resolved against a base, and the base is what chooses the environment.
//!
//! **One function is one call to Plaid.** No function here makes two, and none
//! of them loops — a history that takes four pages is four calls somebody
//! above asked for. Anything else puts a decision about how many times to ask,
//! and in what order, somewhere no state machine can see it, count it or meter
//! it.
//!
//! **Which Plaid is never guessed.** [`PlaidEnv`] has no default and
//! `PLAID_ENV` has to say `sandbox` or `production` in as many words, or the
//! process does not start. See `docs/PLAID.md` for why it matters which:
//! the account allows ten Production Items and removing one does not give the
//! slot back.
//!
//! Everything here needs the secret, so all of it runs on the server. The
//! phone never calls Plaid.

mod plaid_client;
mod plaid_env;
mod plaid_error;
mod plaid_fault;
mod plaid_http_request;
mod plaid_ratelimit;
mod plaid_rejection;
mod plaid_request;
mod plaid_request_accounts;
mod plaid_request_exchange;
mod plaid_request_link_session;
mod plaid_request_link_token;
mod plaid_request_recurring;
mod plaid_request_remove;
mod plaid_request_sandbox_public_token;
mod plaid_request_sync;
mod plaid_request_webhook_key;
mod plaid_response_accounts;
mod plaid_response_exchange;
mod plaid_response_link_session;
mod plaid_response_link_token;
mod plaid_response_recurring;
mod plaid_response_remove;
mod plaid_response_sandbox_public_token;
mod plaid_response_sync;
mod plaid_response_webhook_key;
mod plaid_transaction;
mod plaid_webhook;
mod plaid_webhook_jwt;

pub use plaid_client::*;
pub use plaid_env::*;
pub use plaid_error::*;
pub use plaid_fault::*;
pub use plaid_http_request::*;
pub use plaid_ratelimit::*;
pub use plaid_rejection::*;
pub use plaid_request::*;
pub use plaid_request_accounts::*;
pub use plaid_request_exchange::*;
pub use plaid_request_link_session::*;
pub use plaid_request_link_token::*;
pub use plaid_request_recurring::*;
pub use plaid_request_remove::*;
pub use plaid_request_sandbox_public_token::*;
pub use plaid_request_sync::*;
pub use plaid_request_webhook_key::*;
pub use plaid_response_accounts::*;
pub use plaid_response_exchange::*;
pub use plaid_response_link_session::*;
pub use plaid_response_link_token::*;
pub use plaid_response_recurring::*;
pub use plaid_response_remove::*;
pub use plaid_response_sandbox_public_token::*;
pub use plaid_response_sync::*;
pub use plaid_response_webhook_key::*;
pub use plaid_transaction::*;
pub use plaid_webhook::*;
pub use plaid_webhook_jwt::*;

/// Open a Link session, and get the URL that hosts it.
///
/// The first of the three calls that attach a bank. Nothing is created by this
/// one — a link token is a session, and a session that nobody finishes costs
/// nothing and expires on its own.
pub async fn create_link_token(
    client: &PlaidClient,
    req: &LinkTokenRequest,
) -> Result<LinkTokenResponse, PlaidError> {
    let res: LinkTokenResponse = client.send_request(req).await?;

    tracing::info!(
        update = req.access_token.is_some(),
        hosted = res.hosted_link_url.is_some(),
        expires = res.expiration.as_deref().unwrap_or("unknown"),
        "link session opened",
    );

    Ok(res)
}

/// What became of a Link session.
///
/// The second call, and the one that needs no public URL: the completion
/// redirect closes the sheet but carries no token, so the token is either
/// pushed to a webhook or pulled with this. Poll it until
/// [`LinkSessionResponse::finished`], and read
/// [`LinkSessionResponse::public_tokens`] for the result — which may be empty,
/// because somebody closing the sheet is an outcome and not a failure.
pub async fn link_session(
    client: &PlaidClient,
    link_token: &str,
) -> Result<LinkSessionResponse, PlaidError> {
    let res: LinkSessionResponse = client
        .send_request(&LinkSessionRequest::new(link_token))
        .await?;

    tracing::debug!(
        sessions = res.link_sessions.len(),
        finished = res.finished(),
        tokens = res.public_tokens().len(),
        "link session polled",
    );

    Ok(res)
}

/// The public key one webhook was signed with.
///
/// Caused by an anonymous POST, which is what makes it different from every
/// other call here: the `kid` it asks about came off a token a stranger sent.
/// **The caller caches the answer, misses included.** Without that, anybody
/// who can reach the webhook route can make this server call Plaid at will —
/// and the `alg` check in [`PlaidWebhookJwt::parse`] is what a caller does
/// first so that most junk never gets this far.
pub async fn fetch_webhook_key(client: &PlaidClient, key_id: &str) -> Result<PlaidJwk, PlaidError> {
    let res: WebhookKeyResponse = client.send_request(&WebhookKeyRequest::new(key_id)).await?;

    tracing::info!(key_id, "webhook key fetched");

    Ok(res.key)
}

/// Make an Item in Sandbox without anybody signing into anything.
///
/// The shortcut that keeps development honest rather than fast: what comes
/// back is an ordinary `public_token`, so it is exchanged the ordinary way and
/// the Item behind it syncs like any other. Sandbox only — the endpoint does
/// not exist in Production, so this cannot spend an Item slot.
pub async fn create_sandbox_public_token(
    client: &PlaidClient,
    institution_id: &str,
) -> Result<SandboxPublicTokenResponse, PlaidError> {
    let res: SandboxPublicTokenResponse = client
        .send_request(&SandboxPublicTokenRequest::new(institution_id))
        .await?;

    tracing::info!(institution_id, "sandbox item created");

    Ok(res)
}

/// Trade a `public_token` for the permanent one.
///
/// **The call that spends a Production Item slot.** The account has ten and
/// `/item/remove` does not give one back, so this is the only function in the
/// crate whose effect cannot be undone. What it returns never expires and is
/// stored; see `docs/PLAID.md`.
///
/// The access token is deliberately absent from the log line. It is a
/// credential to somebody's bank, and a log is a file that gets pasted into
/// issues.
pub async fn exchange_public_token(
    client: &PlaidClient,
    public_token: &str,
) -> Result<ExchangeResponse, PlaidError> {
    let res: ExchangeResponse = client
        .send_request(&ExchangeRequest::new(public_token))
        .await?;

    tracing::info!(item_id = res.item_id, "item created");

    Ok(res)
}

/// One page of `/transactions/sync`: everything that changed since `cursor`.
///
/// A page, not a history. Plaid answers `has_more` when there is another one
/// behind it, and asking for that one is the caller's — a loop here would be
/// several calls hidden behind what reads as one, which nothing above could
/// then count, meter or stop halfway.
///
/// The `next_cursor` on the answer is what the next call passes, and it is
/// also the only record of how far this sync got: losing it means re-fetching
/// an entire history.
pub async fn sync_transactions(
    client: &PlaidClient,
    access_token: &str,
    cursor: Option<String>,
) -> Result<SyncResponse, PlaidError> {
    let req = match &cursor {
        Some(cursor) => SyncRequest::after(access_token, cursor),
        None => SyncRequest::initial(access_token),
    };

    let res: SyncResponse = client.send_request(&req).await?;

    tracing::info!(
        added = res.added.len(),
        modified = res.modified.len(),
        removed = res.removed.len(),
        more = res.has_more,
        "transactions synced",
    );

    Ok(res)
}

/// Every account and its balance, from Plaid's cache.
pub async fn fetch_accounts(
    client: &PlaidClient,
    access_token: &str,
) -> Result<AccountsResponse, PlaidError> {
    client
        .send_request(&AccountsRequest::new(access_token))
        .await
}

/// Remove the Item, which is the only way its monthly bill stops.
///
/// **Not reversible.** The access token stops working, and the Production
/// Item slot it spent does not come back — see [`RemoveRequest`]. The caller
/// has decided; this makes the call.
pub async fn remove_item(
    client: &PlaidClient,
    access_token: &str,
) -> Result<RemoveResponse, PlaidError> {
    let res = client.send_request(&RemoveRequest::new(access_token)).await;

    match &res {
        Ok(_) => tracing::info!("item removed; its plaid subscription stops"),
        Err(e) => tracing::error!(error = %e, "an item was not removed, and is still billing"),
    }

    res
}

/// The recurring streams Plaid has detected on this Item.
pub async fn fetch_recurring(
    client: &PlaidClient,
    access_token: &str,
) -> Result<RecurringResponse, PlaidError> {
    client
        .send_request(&RecurringRequest::new(access_token))
        .await
}
