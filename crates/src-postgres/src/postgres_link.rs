//! Link sessions that have been opened and not yet collected.

use api_common::Timestamp;
use uuid::Uuid;

use crate::Db;
use crate::PostgresError;
use crate::postgres_ledger::seconds;

/// A Link session waiting to be collected.
#[derive(Debug, Clone)]
pub struct PendingLink {
    pub link_token: String,
    pub user_id: Uuid,
    pub created_at: Timestamp,
}

/// Remember that this user opened this Link session.
///
/// Written before the URL is handed back, so there is no window in which
/// somebody could finish a session we have no record of asking for.
pub async fn save_link(db: &Db, link_token: &str, user_id: Uuid) -> Result<(), PostgresError> {
    sqlx::query!(
        "INSERT INTO link_sessions (link_token, user_id) VALUES ($1, $2)
         ON CONFLICT (link_token) DO NOTHING",
        link_token,
        user_id,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// This user's uncollected Link sessions, newest first.
///
/// Scoped to the user, which is the whole reason the table exists: collecting
/// is what turns a session into an Item, and an Item is somebody's bank.
pub async fn pending_links(db: &Db, user_id: Uuid) -> Result<Vec<PendingLink>, PostgresError> {
    let rows = sqlx::query!(
        "SELECT link_token, user_id, created_at FROM link_sessions
         WHERE user_id = $1 AND collected_at IS NULL
         ORDER BY created_at DESC",
        user_id,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PendingLink {
            link_token: row.link_token,
            user_id: row.user_id,
            created_at: seconds(row.created_at),
        })
        .collect())
}

/// Who opened this Link session.
///
/// The only thing that says which account a newly linked bank belongs to. The
/// row was written before the session's URL was handed over, so there is no
/// window in which a bank could be collected against nobody.
pub async fn user_for_link(db: &Db, link_token: &str) -> Result<Option<Uuid>, PostgresError> {
    let row = sqlx::query!(
        "SELECT user_id FROM link_sessions WHERE link_token = $1",
        link_token,
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| row.user_id))
}

/// Mark a session collected, whatever it turned out to hold.
///
/// Set on an empty session too — somebody closing the sheet is an outcome, and
/// a session that stays pending forever is one we would poll forever.
pub async fn collect_link(db: &Db, link_token: &str) -> Result<(), PostgresError> {
    sqlx::query!(
        "UPDATE link_sessions SET collected_at = now() WHERE link_token = $1",
        link_token,
    )
    .execute(db)
    .await?;

    Ok(())
}
