//! The people the banks belong to, and how they prove it.

use uuid::Uuid;

use crate::Db;
use crate::PostgresError;

/// Find the user who signs in with this address, or make one.
///
/// The lookup is on `identities`, not on `users`: a credential is a row, so
/// that the same person arriving later by Apple or Google is another row
/// against the same user rather than a second account. `users.email` is the
/// address we write *to*, which is a different question and may drift from the
/// one they sign in with.
///
/// Matched on `lower(email)`, because nobody thinks of `Alex@example.com` as a
/// second account. Stored as typed, because the capital is theirs.
///
/// One transaction, so two requests racing on a new address cannot leave a
/// user with no way to sign in: the loser's identity insert hits the primary
/// key and takes its half-made user down with it.
pub async fn upsert_user(db: &Db, email: &str) -> Result<Uuid, PostgresError> {
    let subject = email.trim().to_lowercase();

    let mut tx = db.begin().await?;

    let existing = sqlx::query!(
        "SELECT user_id FROM identities WHERE provider = 'email' AND subject = $1",
        subject,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let user_id = match existing {
        Some(row) => row.user_id,

        None => {
            let row = sqlx::query!(
                "INSERT INTO users (email) VALUES ($1) RETURNING user_id",
                email,
            )
            .fetch_one(&mut *tx)
            .await?;

            sqlx::query!(
                "INSERT INTO identities (provider, subject, user_id)
                 VALUES ('email', $1, $2)",
                subject,
                row.user_id,
            )
            .execute(&mut *tx)
            .await?;

            row.user_id
        }
    };

    tx.commit().await?;

    Ok(user_id)
}

/// The address a user is written to.
///
/// What Stripe emails a receipt to. Nothing the state machine holds says who a
/// user is besides their id, so this is the read that answers it.
pub async fn load_email(db: &Db, user_id: Uuid) -> Result<Option<String>, PostgresError> {
    let row = sqlx::query!("SELECT email FROM users WHERE user_id = $1", user_id)
        .fetch_optional(db)
        .await?;

    Ok(row.map(|row| row.email))
}
