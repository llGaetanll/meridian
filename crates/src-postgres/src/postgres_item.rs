//! The Items: one row per bank connection.
//!
//! Everything here is the source's own fact about a connection — the token,
//! the cursor, the institution, whether it is still healthy. What a *screen*
//! makes of that is somebody else's business.

use api_common::Timestamp;
use uuid::Uuid;

use crate::Db;
use crate::PostgresError;
use crate::postgres_ledger::seconds;

/// One bank connection, as it is stored.
///
/// Not [`api_ledger::Connection`], which is the same connection *projected* for
/// a screen — that one carries initials and a worded status and no access
/// token. This is the record; that is a view of it.
#[derive(Debug, Clone)]
pub struct StoredItem {
    pub item_id: String,

    /// Whose bank this is. The column every read scopes on, so it is not
    /// optional and never inferred.
    pub user_id: Uuid,

    /// Permanent, and a live credential to somebody's bank. Never logged,
    /// never sent to a client, and encrypted before the first Production Item
    /// — see issue #21.
    pub access_token: String,

    pub institution_id: Option<String>,

    pub institution_name: Option<String>,

    /// Where the last sync reached. `None` means the next one starts from the
    /// beginning, which is correct and merely slow.
    pub cursor: Option<String>,

    /// The source's error code when the connection needs a person, most often
    /// `ITEM_LOGIN_REQUIRED`. `None` is healthy.
    pub error_code: Option<String>,

    /// When this row was last written, which is as good a "last synced" as
    /// exists — nothing writes it except a sync.
    pub synced_at: Timestamp,
}

/// Write an Item, replacing whatever was there.
///
/// An upsert rather than an insert: linking the same bank twice is something
/// people do, and the source hands back the same `item_id` for it. Two columns
/// are deliberately *not* in the update — the cursor, which moves only through
/// [`save_cursor`] so a re-save cannot rewind a sync, and `user_id`, so that
/// re-linking cannot move somebody else's bank onto your account.
pub async fn save_item(db: &Db, item: &StoredItem) -> Result<(), PostgresError> {
    sqlx::query!(
        "INSERT INTO items (item_id, user_id, access_token, institution_id, institution_name, error_code, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, now())
         ON CONFLICT (item_id) DO UPDATE SET
             access_token     = EXCLUDED.access_token,
             institution_id   = EXCLUDED.institution_id,
             institution_name = EXCLUDED.institution_name,
             error_code       = EXCLUDED.error_code,
             updated_at       = now()",
        item.item_id,
        item.user_id,
        item.access_token,
        item.institution_id,
        item.institution_name,
        item.error_code,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Record what the bank is called, as the balances call reported it.
///
/// Its own function rather than part of the upsert, so a sync cannot touch
/// `user_id` or `access_token` at all — a re-save must never be able to move
/// somebody's bank onto another account or blank its credential.
pub async fn name_item(
    db: &Db,
    item_id: &str,
    institution_name: Option<&str>,
) -> Result<(), PostgresError> {
    sqlx::query!(
        "UPDATE items SET institution_name = $2, updated_at = now() WHERE item_id = $1",
        item_id,
        institution_name,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// One user's Items, oldest first, so the connection list is stable across
/// restarts.
///
/// Scoped, and there is no unscoped version on purpose. This query used to have
/// no `WHERE` at all, which read as "everything I know" while there was one
/// user and as "everyone's banks" the moment there were two.
pub async fn load_items(db: &Db, user_id: Uuid) -> Result<Vec<StoredItem>, PostgresError> {
    let rows = sqlx::query!(
        "SELECT item_id, user_id, access_token, institution_id, institution_name, cursor, error_code, updated_at
         FROM items
         WHERE user_id = $1
         ORDER BY created_at",
        user_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| StoredItem {
            item_id: row.item_id,
            user_id: row.user_id,
            access_token: row.access_token,
            institution_id: row.institution_id,
            institution_name: row.institution_name,
            cursor: row.cursor,
            error_code: row.error_code,
            synced_at: seconds(row.updated_at),
        })
        .collect())
}

/// Move the cursor.
///
/// Its own function, and the only thing that writes the column. The cursor is
/// the single record of how far a sync got: losing it re-fetches an entire
/// history, and rewinding it re-adds transactions the ledger already holds.
/// Keeping it out of [`save_item`] means neither can happen by accident.
pub async fn save_cursor(db: &Db, item_id: &str, cursor: &str) -> Result<(), PostgresError> {
    sqlx::query!(
        "UPDATE items SET cursor = $2, updated_at = now() WHERE item_id = $1",
        item_id,
        cursor,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Record that a connection needs a person, or that it no longer does.
pub async fn save_error(
    db: &Db,
    item_id: &str,
    error_code: Option<&str>,
) -> Result<(), PostgresError> {
    sqlx::query!(
        "UPDATE items SET error_code = $2, updated_at = now() WHERE item_id = $1",
        item_id,
        error_code,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Forget a bank, and everything that came through it.
///
/// The accounts, transactions and streams go with it, by the cascade the
/// schema declares. That is what keeps the promise the app makes on the screen
/// where it asks to connect one: removing a bank deletes its history.
pub async fn remove_item(db: &Db, item_id: &str) -> Result<(), PostgresError> {
    sqlx::query!("DELETE FROM items WHERE item_id = $1", item_id)
        .execute(db)
        .await?;

    tracing::info!(item_id, "item removed with its history");

    Ok(())
}
