//! Sessions: what the phone sends instead of signing in again.

use api_common::Timestamp;
use uuid::Uuid;

use crate::Db;
use crate::PostgresError;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// Store a session and say which user it signs in.
///
/// The caller passes the **hash**, never the token. That asymmetry is the
/// point: the token exists in one place, on the phone, and a copy of this
/// table is a list of who has logged in rather than a way to become them.
pub async fn create_session(
    db: &Db,
    token_sha256: &[u8],
    user_id: Uuid,
    expires_at: Option<Timestamp>,
) -> Result<(), PostgresError> {
    sqlx::query!(
        "INSERT INTO sessions (token_sha256, user_id, expires_at) VALUES ($1, $2, $3)",
        token_sha256,
        user_id,
        expires_at.map(stamp),
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Who this token signs in, if anybody.
///
/// Expiry is applied here rather than by the caller. It is a property of the
/// row, an expired session is indistinguishable from no session at the only
/// place that matters, and leaving it to a caller means every caller has to
/// remember.
pub async fn user_for_session(
    db: &Db,
    token_sha256: &[u8],
    now: Timestamp,
) -> Result<Option<Uuid>, PostgresError> {
    let row = sqlx::query!(
        "SELECT user_id FROM sessions
         WHERE token_sha256 = $1 AND (expires_at IS NULL OR expires_at > $2)",
        token_sha256,
        stamp(now),
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| row.user_id))
}

/// Sign out. A DELETE rather than a flag, which is the whole reason a session
/// is a row and not a signed token.
pub async fn delete_session(db: &Db, token_sha256: &[u8]) -> Result<(), PostgresError> {
    sqlx::query!("DELETE FROM sessions WHERE token_sha256 = $1", token_sha256)
        .execute(db)
        .await?;

    Ok(())
}

/// Drop every session that has already expired.
///
/// Nothing calls this on a timer yet. It exists so that the table has a stated
/// way to be swept, rather than the question being discovered later.
pub async fn sweep_sessions(db: &Db, now: Timestamp) -> Result<u64, PostgresError> {
    let done = sqlx::query!(
        "DELETE FROM sessions WHERE expires_at IS NOT NULL AND expires_at <= $1",
        stamp(now),
    )
    .execute(db)
    .await?;

    Ok(done.rows_affected())
}

/// Reachable so the session tests can read a row's expiry back.
pub async fn session_expiry(
    db: &Db,
    token_sha256: &[u8],
) -> Result<Option<Timestamp>, PostgresError> {
    let row = sqlx::query!(
        "SELECT expires_at FROM sessions WHERE token_sha256 = $1",
        token_sha256,
    )
    .fetch_optional(db)
    .await?;

    Ok(row.and_then(|row| row.expires_at).map(seconds))
}
