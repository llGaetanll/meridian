//! The codes that have been sent and not yet used.

use api_common::Timestamp;

use crate::Db;
use crate::PostgresError;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// A code as it is stored. The hash, when it dies, and how many guesses it has
/// taken — everything [`login_code::verify`] needs and nothing else.
#[derive(Debug, Clone)]
pub struct StoredCode {
    pub code_sha256: Vec<u8>,
    pub expires_at: Timestamp,
    pub attempts: i32,
}

/// Put a code against an address, replacing any that was there.
///
/// One live code per address, so asking again invalidates the last one. The
/// alternative is a table of every code ever sent, which is a table of guesses
/// somebody can work through — and a user who asks twice and then types the
/// first code should be told no.
///
/// `attempts` resets with the row, which is what makes "send me another"
/// a genuine way out of a run of wrong guesses.
pub async fn save_code(
    db: &Db,
    email: &str,
    code_sha256: &[u8],
    expires_at: Timestamp,
) -> Result<(), PostgresError> {
    sqlx::query!(
        "INSERT INTO login_codes (email, code_sha256, expires_at, attempts, created_at)
         VALUES ($1, $2, $3, 0, now())
         ON CONFLICT (email) DO UPDATE SET
             code_sha256 = EXCLUDED.code_sha256,
             expires_at  = EXCLUDED.expires_at,
             attempts    = 0,
             created_at  = now()",
        email,
        code_sha256,
        stamp(expires_at),
    )
    .execute(db)
    .await?;

    Ok(())
}

/// The code standing against this address, if there is one.
pub async fn load_code(db: &Db, email: &str) -> Result<Option<StoredCode>, PostgresError> {
    let row = sqlx::query!(
        "SELECT code_sha256, expires_at, attempts FROM login_codes WHERE email = $1",
        email,
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| StoredCode {
        code_sha256: row.code_sha256,
        expires_at: seconds(row.expires_at),
        attempts: row.attempts,
    }))
}

/// Count a wrong guess.
///
/// Incremented in the database rather than read, added to and written back:
/// two guesses arriving at once would otherwise both read the same number and
/// both write it, and the cap would be one guess wider than it says.
pub async fn record_attempt(db: &Db, email: &str) -> Result<(), PostgresError> {
    sqlx::query!(
        "UPDATE login_codes SET attempts = attempts + 1 WHERE email = $1",
        email,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Spend the code. Single use, so this is what a successful sign-in does with
/// it — and what makes a replayed code worth nothing.
pub async fn delete_code(db: &Db, email: &str) -> Result<(), PostgresError> {
    sqlx::query!("DELETE FROM login_codes WHERE email = $1", email)
        .execute(db)
        .await?;

    Ok(())
}
