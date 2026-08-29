//! The people the banks belong to.

use uuid::Uuid;

use crate::Db;
use crate::PostgresError;

/// Find the user with this address, or make one.
///
/// Matched on `lower(email)`, which is what the unique index is on: nobody
/// thinks of `Alex@example.com` as a second account. The address is stored as
/// it was typed, because the capital is theirs.
pub async fn upsert_user(db: &Db, email: &str) -> Result<Uuid, PostgresError> {
    // `DO UPDATE` rather than `DO NOTHING`: nothing needs updating, but only
    // an update makes the conflicting row visible to RETURNING, and a second
    // SELECT to find what we just conflicted with is a race.
    let row = sqlx::query!(
        "INSERT INTO users (email) VALUES ($1)
         ON CONFLICT (lower(email)) DO UPDATE SET email = users.email
         RETURNING user_id",
        email,
    )
    .fetch_one(db)
    .await?;

    Ok(row.user_id)
}
