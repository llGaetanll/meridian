//! The people the banks belong to, and how they prove it.

use uuid::Uuid;

use crate::Db;
use crate::DbKey;
use crate::PostgresError;
use crate::postgres_secret;

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

/// Who a user is, as far as anything here knows: the address, and the name if
/// they have given one.
///
/// Both in one read because both are printed in the same place — the account
/// chip — and two queries for two columns of one row is two ways for the app
/// to be holding half an answer.
pub struct StoredUser {
    pub email: String,

    /// Absent for every account made before the name was asked for, and for
    /// anyone who has not been asked yet.
    pub name: Option<String>,
}

pub async fn load_user(db: &Db, user_id: Uuid) -> Result<Option<StoredUser>, PostgresError> {
    let row = sqlx::query!("SELECT email, name FROM users WHERE user_id = $1", user_id)
        .fetch_optional(db)
        .await?;

    Ok(row.map(|row| StoredUser {
        email: row.email,
        name: row.name,
    }))
}

/// Set the name, or clear it.
///
/// Trimmed, and blank means absent: a field somebody tabbed through leaves the
/// column NULL rather than a name made of spaces, so "has a name" stays one
/// question with one answer.
pub async fn save_name(db: &Db, user_id: Uuid, name: &str) -> Result<(), PostgresError> {
    let name = name.trim();
    let name = (!name.is_empty()).then_some(name);

    sqlx::query!(
        "UPDATE users SET name = $2 WHERE user_id = $1",
        user_id,
        name,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Every access token this user's banks were reached with, for the calls that
/// stop Plaid billing for them.
///
/// Read before the rows go, because deleting the user takes `items` with it and
/// a token nobody holds is a bank that bills forever. The tokens are opened on
/// the way out, so what this answers is a credential — it goes to Plaid and
/// nowhere else.
pub async fn load_access_tokens(
    db: &Db,
    user_id: Uuid,
    key: &DbKey,
) -> Result<Vec<(String, String)>, PostgresError> {
    let rows = sqlx::query!(
        "SELECT item_id, access_token FROM items WHERE user_id = $1",
        user_id,
    )
    .fetch_all(db)
    .await?;

    let mut tokens = Vec::with_capacity(rows.len());

    for row in rows {
        // A token that will not open is one nothing can remove the Item with.
        // Skipped rather than failing the whole deletion: the rows still go,
        // and the alternative is an account nobody can delete.
        match postgres_secret::open(key, &row.access_token) {
            Ok(token) => tokens.push((row.item_id, token)),
            Err(e) => tracing::error!(
                item_id = row.item_id,
                error = %e,
                "an access token would not open; its item cannot be removed at plaid",
            ),
        }
    }

    Ok(tokens)
}

/// Delete the account, and everything that hangs off it.
///
/// `users` is the root: sessions, identities, items, and — through items —
/// accounts, entries and streams are all `ON DELETE CASCADE` from it, as is
/// the subscription row. So this is one statement and the schema does the rest.
///
/// `login_codes` is the exception, because it is keyed by address rather than
/// by user and nothing ties it to one. Deleted here by address, so a code in
/// flight cannot outlive the account it was for.
///
/// **What this does not do is Plaid.** Removing the Items is a call, not a
/// write, and it has to happen while the tokens are still readable — see
/// [`load_access_tokens`]. A caller that deletes without removing has stopped
/// the app and not the bill.
pub async fn delete_user(db: &Db, user_id: Uuid) -> Result<(), PostgresError> {
    let mut tx = db.begin().await?;

    // By address, and before the user goes: after it, there is nothing left to
    // find the address by.
    sqlx::query!(
        "DELETE FROM login_codes
         WHERE email IN (SELECT lower(email) FROM users WHERE user_id = $1)",
        user_id,
    )
    .execute(&mut *tx)
    .await?;

    // Every credential this account authenticated with, which is what makes
    // the address free for a fresh signup.
    sqlx::query!("DELETE FROM identities WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!("DELETE FROM users WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    tracing::info!(%user_id, "account deleted");

    Ok(())
}
