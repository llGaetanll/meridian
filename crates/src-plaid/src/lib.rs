//! Plaid, the bank connection.
//!
//! Like `src-yahoo` and `src-coingecko` this is IO only: plain async functions
//! over plain data, no signals and no decisions. Requests carry a path
//! resolved against a base, and the base is what chooses the environment.
//!
//! **Sandbox unless told otherwise.** [`PEnv`] defaults to it and `PLAID_ENV`
//! has to say `production` in as many words to get anything else. See
//! `docs/PLAID.md` for why: the account allows ten Production Items and
//! removing one does not give the slot back.
//!
//! Everything here needs the secret, so all of it runs on the server. The
//! phone never calls Plaid.

mod p_env;
mod p_error;
mod p_request;
mod p_request_sync;
mod p_response_sync;
mod plaid_client;
mod transaction;

pub use p_env::*;
pub use p_error::*;
pub use p_request::*;
pub use p_request_sync::*;
pub use p_response_sync::*;
pub use plaid_client::*;
pub use transaction::*;

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
) -> Result<Sync, PError> {
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
        target: "app",
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
