//! Plaid, the bank connection.
//!
//! Like `src-yahoo` and `src-coingecko` this is IO only: plain async functions
//! over plain data, no signals and no decisions. Requests carry a path
//! resolved against a base, and the base is what chooses the environment.
//!
//! **Sandbox unless told otherwise.** [`PlaidEnv`] defaults to it and `PLAID_ENV`
//! has to say `production` in as many words to get anything else. See
//! `docs/PLAID.md` for why: the account allows ten Production Items and
//! removing one does not give the slot back.
//!
//! Everything here needs the secret, so all of it runs on the server. The
//! phone never calls Plaid.

mod plaid_client;
mod plaid_env;
mod plaid_error;
mod plaid_http_request;
mod plaid_ratelimit;
mod plaid_request;
mod plaid_request_balances;
mod plaid_request_exchange;
mod plaid_request_link_session;
mod plaid_request_link_token;
mod plaid_request_recurring;
mod plaid_request_sandbox_public_token;
mod plaid_request_sync;
mod plaid_response_balances;
mod plaid_response_exchange;
mod plaid_response_link_session;
mod plaid_response_link_token;
mod plaid_response_recurring;
mod plaid_response_sandbox_public_token;
mod plaid_response_sync;
mod plaid_transaction;

pub use plaid_client::*;
pub use plaid_env::*;
pub use plaid_error::*;
pub use plaid_http_request::*;
pub use plaid_ratelimit::*;
pub use plaid_request::*;
pub use plaid_request_balances::*;
pub use plaid_request_exchange::*;
pub use plaid_request_link_session::*;
pub use plaid_request_link_token::*;
pub use plaid_request_recurring::*;
pub use plaid_request_sandbox_public_token::*;
pub use plaid_request_sync::*;
pub use plaid_response_balances::*;
pub use plaid_response_exchange::*;
pub use plaid_response_link_session::*;
pub use plaid_response_link_token::*;
pub use plaid_response_recurring::*;
pub use plaid_response_sandbox_public_token::*;
pub use plaid_response_sync::*;
pub use plaid_transaction::*;

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

/// Every transaction since `cursor`, following Plaid's paging to the end.
///
/// Returns what changed and the cursor to resume from next time. The caller
/// saves that cursor: it is the only record of how far the last sync got, and
/// losing it means re-fetching the whole history.
///
/// Paging is here rather than at the call site because a half-followed page
/// chain is a silently short answer — the cursor would still advance, and the
/// skipped transactions would never be asked for again.
pub async fn sync_transactions(
    client: &PlaidClient,
    access_token: &str,
    cursor: Option<String>,
) -> Result<Sync, PlaidError> {
    let mut sync = Sync {
        cursor: cursor.unwrap_or_default(),
        ..Default::default()
    };

    loop {
        let req = if sync.cursor.is_empty() {
            SyncRequest::initial(access_token)
        } else {
            SyncRequest::after(access_token, &sync.cursor)
        };

        let res: SyncResponse = client.send_request(&req).await?;

        sync.added.extend(res.added);
        sync.modified.extend(res.modified);
        sync.removed
            .extend(res.removed.into_iter().map(|r| r.transaction_id));
        sync.cursor = res.next_cursor;

        if !res.has_more {
            break;
        }
    }

    tracing::info!(
        added = sync.added.len(),
        modified = sync.modified.len(),
        removed = sync.removed.len(),
        "transactions synced",
    );

    Ok(sync)
}

/// Everything one sync turned up, pages already joined.
#[derive(Debug, Default, Clone)]
pub struct Sync {
    pub added: Vec<PlaidTransaction>,
    pub modified: Vec<PlaidTransaction>,

    /// Ids only — these transactions no longer exist.
    pub removed: Vec<String>,

    /// Where to resume next time.
    pub cursor: String,
}

/// Every account's balance, straight from the bank.
pub async fn fetch_balances(
    client: &PlaidClient,
    access_token: &str,
) -> Result<BalancesResponse, PlaidError> {
    client
        .send_request(&BalancesRequest::new(access_token))
        .await
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
