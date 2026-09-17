//! Link sessions, against a real Postgres: that a repair session remembers
//! which Item it repairs, and an ordinary one remembers that it does not.
//!
//! The distinction is what keeps a repair's finish from being exchanged for a
//! new Item, so it has to survive the write and come back on both reads —
//! the sheet-close's list and the webhook's lookup.

use api_common::Timestamp;
use sqlx::PgPool;
use src_postgres::StoredItem;

/// Thirty-two nothings. There is no secret here to keep.
fn key() -> src_postgres::DbKey {
    src_postgres::DbKey::parse("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
        .expect("thirty-two bytes is a key")
}

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_repair_session_remembers_its_item_and_an_ordinary_one_does_not(db: PgPool) {
    let user_id = src_postgres::upsert_user(&db, "al@example.com")
        .await
        .expect("the user is written");

    src_postgres::save_item(
        &db,
        &key(),
        &StoredItem {
            item_id: "item-9".to_string(),
            user_id,
            access_token: "access-sandbox-9".to_string(),
            institution_id: None,
            institution_name: None,
            cursor: None,
            error_code: Some("ITEM_LOGIN_REQUIRED".to_string()),
            synced_at: Timestamp::EPOCH,
        },
    )
    .await
    .expect("the bank is written");

    src_postgres::save_link(&db, "link-attach", user_id, None)
        .await
        .expect("the ordinary session is written");

    src_postgres::save_link(&db, "link-repair", user_id, Some("item-9"))
        .await
        .expect("the repair session is written");

    let pending = src_postgres::pending_links(&db, user_id)
        .await
        .expect("the open sessions are read");

    assert_eq!(pending.len(), 2);

    let repair_of = |token: &str| {
        pending
            .iter()
            .find(|link| link.link_token == token)
            .map(|link| link.repair_of.clone())
            .expect("the session is in the list")
    };

    assert_eq!(repair_of("link-attach"), None);
    assert_eq!(repair_of("link-repair"), Some("item-9".to_string()));

    // The webhook's lookup, by token alone. A token with no row is the same
    // answer as an ordinary session, because neither may skip the exchange.
    assert_eq!(
        src_postgres::repair_for_link(&db, "link-repair")
            .await
            .expect("the lookup answers"),
        Some("item-9".to_string()),
    );
    assert_eq!(
        src_postgres::repair_for_link(&db, "link-attach")
            .await
            .expect("the lookup answers"),
        None,
    );
    assert_eq!(
        src_postgres::repair_for_link(&db, "link-unknown")
            .await
            .expect("the lookup answers"),
        None,
    );
}
