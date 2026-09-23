//! The audit log: who read customer data, written down where it is kept.
//!
//! Not the app logs. A log line rotates away and says what the process did;
//! a row here answers "who has seen this person's data" for as long as the
//! question can be asked. The table refuses UPDATE and DELETE, so a row only
//! ever arrives.

use api_common::Timestamp;
use uuid::Uuid;

use crate::Db;
use crate::PostgresError;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// What a read served, named for the row that records it.
///
/// The server's readers are all here; a backup writes its row itself, as
/// `backup`, because it reads with `pg_dump` rather than through this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditScope {
    /// The ledger, whole: accounts, transactions, streams.
    Snapshot,

    /// The subscription: whether they pay, until when, through whom.
    Billing,

    /// The sync stream: which bank is being fetched and how far it has got.
    Sync,
}

impl AuditScope {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditScope::Snapshot => "snapshot",
            AuditScope::Billing => "billing",
            AuditScope::Sync => "sync",
        }
    }
}

/// One access, read back.
#[derive(Debug)]
pub struct StoredAccess {
    pub at: Timestamp,
    pub user_id: Option<Uuid>,
    pub scope: String,
    pub client: String,
}

/// Write down that this user's data was served.
pub async fn record_access(
    db: &Db,
    user_id: Uuid,
    scope: AuditScope,
    client: &str,
    at: Timestamp,
) -> Result<(), PostgresError> {
    sqlx::query!(
        "INSERT INTO audit_log (at, user_id, scope, client) VALUES ($1, $2, $3, $4)",
        stamp(at),
        user_id,
        scope.as_str(),
        client,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Every access to this user's data, oldest first.
///
/// Reachable so the audit tests can read rows back; producing a person's
/// access history would start here too.
pub async fn load_accesses(db: &Db, user_id: Uuid) -> Result<Vec<StoredAccess>, PostgresError> {
    let rows = sqlx::query!(
        "SELECT at, user_id, scope, client FROM audit_log
         WHERE user_id = $1 ORDER BY at",
        user_id,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| StoredAccess {
            at: seconds(row.at),
            user_id: row.user_id,
            scope: row.scope,
            client: row.client,
        })
        .collect())
}
