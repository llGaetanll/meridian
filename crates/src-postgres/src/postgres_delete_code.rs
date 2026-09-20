//! The codes that confirm an account deletion.
//!
//! The same shape as `postgres_login`, keyed by the user instead of the
//! address: the route that asks is authenticated, and a deletion code in
//! `login_codes` would overwrite a sign-in half way through.

use api_common::Timestamp;
use uuid::Uuid;

use crate::Db;
use crate::PostgresError;
use crate::StoredCode;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// Put a code against a user, replacing any that was there.
///
/// One live code per account, so asking again invalidates the last one, and
/// `attempts` resets with the row — the rules `login_codes` follows, for the
/// reasons written there.
pub async fn save_delete_code(
    db: &Db,
    user_id: Uuid,
    code_sha256: &[u8],
    expires_at: Timestamp,
) -> Result<(), PostgresError> {
    sqlx::query!(
        "INSERT INTO delete_codes (user_id, code_sha256, expires_at, attempts, created_at)
         VALUES ($1, $2, $3, 0, now())
         ON CONFLICT (user_id) DO UPDATE SET
             code_sha256 = EXCLUDED.code_sha256,
             expires_at  = EXCLUDED.expires_at,
             attempts    = 0,
             created_at  = now()",
        user_id,
        code_sha256,
        stamp(expires_at),
    )
    .execute(db)
    .await?;

    Ok(())
}

/// The code standing against this user, if there is one.
pub async fn load_delete_code(db: &Db, user_id: Uuid) -> Result<Option<StoredCode>, PostgresError> {
    let row = sqlx::query!(
        "SELECT code_sha256, expires_at, attempts FROM delete_codes WHERE user_id = $1",
        user_id,
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| StoredCode {
        code_sha256: row.code_sha256,
        expires_at: seconds(row.expires_at),
        attempts: row.attempts,
    }))
}

/// Count a wrong guess. Incremented in the database for the reason
/// `record_attempt` gives: two guesses at once must not share a read.
pub async fn record_delete_attempt(db: &Db, user_id: Uuid) -> Result<(), PostgresError> {
    sqlx::query!(
        "UPDATE delete_codes SET attempts = attempts + 1 WHERE user_id = $1",
        user_id,
    )
    .execute(db)
    .await?;

    Ok(())
}
