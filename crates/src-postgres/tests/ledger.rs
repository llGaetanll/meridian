//! The queries, against a real Postgres.
//!
//! `#[sqlx::test]` gives each of these its **own database**: it creates one,
//! runs our migrator into it, hands over a pool, and drops it afterwards. So
//! they are isolated from each other and — the part that matters — from the
//! database you actually work in. Nothing here can touch a real access token.
//!
//! A failing test leaves its database behind on purpose, so you can go and
//! look at what it left. sqlx sweeps those on the next run.
//!
//! They need a server to create those databases on:
//!
//! ```sh
//! ./scripts/db-init.sh
//! cargo test
//! ```

use api_common::Timestamp;
use api_ledger::Account;
use api_ledger::AccountGroup;
use api_ledger::Category;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Stream;
use api_ledger::Tx;
use sqlx::PgPool;
use src_postgres::StoredItem;
use uuid::Uuid;

const ITEM: &str = "item-test";

/// 2026-08-19, the day the fixtures are drawn against.
const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

/// The key the sealed access-token column is written with.
///
/// Fixed, and thirty-two nothings: a test wants the same answer twice, and
/// there is no secret here to keep. The real one comes from `MERIDIAN_DB_KEY`.
fn key() -> src_postgres::DbKey {
    src_postgres::DbKey::parse("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
        .expect("thirty-two bytes is a key")
}

/// The owner every test's data hangs off. Made per test, because
/// `#[sqlx::test]` gives each one its own database and a uuid is the
/// database's to hand out.
async fn user(db: &PgPool) -> Uuid {
    src_postgres::upsert_user(db, "test@example.com")
        .await
        .unwrap()
}

fn item(user_id: Uuid) -> StoredItem {
    StoredItem {
        item_id: ITEM.to_string(),
        user_id,
        access_token: "access-sandbox-1".to_string(),
        institution_id: Some("ins_109508".to_string()),
        institution_name: Some("First Platypus Bank".to_string()),
        cursor: None,
        error_code: None,
        synced_at: NOW,
    }
}

fn entry(id: &str, amount: f64) -> Entry {
    Entry {
        id: id.to_string(),
        tx: Tx::Transfer {
            ts: NOW,
            pos: Position {
                asset: "USD".to_string(),
                qty: amount,
            },
        },
        label: "Uber".to_string(),
        account_id: Some("acc-1".to_string()),
        logo_url: None,
        category: Category::Transportation,
        pending: false,
        supersedes: None,
    }
}

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn an_item_round_trips(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let items = src_postgres::load_items(&db, &key(), user_id)
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_id, ITEM);
    assert_eq!(items[0].access_token, "access-sandbox-1");
    assert_eq!(
        items[0].institution_name.as_deref(),
        Some("First Platypus Bank")
    );
    assert_eq!(items[0].cursor, None, "a new item has read nothing yet");
}

/// The cursor is the only record of how far a sync got. Re-saving an Item —
/// which happens whenever a bank is relinked — must not rewind it, or the next
/// sync re-fetches a history the ledger already holds.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn saving_an_item_again_does_not_rewind_the_cursor(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();
    src_postgres::save_cursor(&db, ITEM, "cursor-abc")
        .await
        .unwrap();

    // Again, as a relink does.
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let items = src_postgres::load_items(&db, &key(), user_id)
        .await
        .unwrap();

    assert_eq!(items[0].cursor.as_deref(), Some("cursor-abc"));
}

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn accounts_round_trip_with_their_money_intact(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let accounts = vec![Account {
        id: "acc-1".to_string(),
        name: "Plaid Checking".to_string(),
        subtype: Some("checking".to_string()),
        mask: Some("0000".to_string()),
        balance: 110.0,
        available: Some(100.0),
        limit: None,
        apy: None,
        group: AccountGroup::Depository,
    }];

    src_postgres::save_accounts(&db, ITEM, &accounts)
        .await
        .unwrap();

    let back = src_postgres::load_accounts(&db, ITEM).await.unwrap();

    assert_eq!(back, accounts, "an account should come back as itself");
    assert!(!back[0].is_debt());
}

/// The bug that made one transaction appear three times: a sync that re-sends
/// an entry the ledger already holds. The upsert is what makes applying the
/// same diff twice a no-op.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn applying_the_same_entries_twice_stores_them_once(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let added = vec![entry("a", -25.0), entry("b", -4.33)];

    src_postgres::apply_entries(&db, ITEM, &added, &[], &[])
        .await
        .unwrap();
    src_postgres::apply_entries(&db, ITEM, &added, &[], &[])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db, user_id).await.unwrap();

    assert_eq!(entries.len(), 2, "a transaction appears once");
    assert_eq!(entries.iter().filter(|e| e.id == "a").count(), 1);
}

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_modified_entry_replaces_its_old_self(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    src_postgres::apply_entries(&db, ITEM, &[entry("a", -25.0)], &[], &[])
        .await
        .unwrap();

    let mut changed = entry("a", -30.0);
    changed.label = "Uber Eats".to_string();

    src_postgres::apply_entries(&db, ITEM, &[], &[changed], &[])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db, user_id).await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].amount(), -30.0);
    assert_eq!(entries[0].label, "Uber Eats");
}

/// Trap two from docs/PLAID.md. A posted transaction arrives with a new id
/// pointing at the pending one it replaces, and unless the pending one goes
/// every purchase is counted twice for the days between the two.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_posted_entry_drops_the_pending_one_it_replaces(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let mut pending = entry("pending-1", -25.0);
    pending.pending = true;

    src_postgres::apply_entries(&db, ITEM, &[pending], &[], &[])
        .await
        .unwrap();

    let mut posted = entry("posted-1", -25.0);
    posted.supersedes = Some("pending-1".to_string());

    src_postgres::apply_entries(&db, ITEM, &[posted], &[], &[])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db, user_id).await.unwrap();

    assert_eq!(entries.len(), 1, "counted once, not twice");
    assert_eq!(entries[0].id, "posted-1");
}

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_retracted_entry_is_removed(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    src_postgres::apply_entries(&db, ITEM, &[entry("a", -25.0), entry("b", -4.33)], &[], &[])
        .await
        .unwrap();

    src_postgres::apply_entries(&db, ITEM, &[], &[], &["a".to_string()])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db, user_id).await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "b");
}

/// Newest first, which is the order every screen shows.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn entries_come_back_newest_first(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let older = Entry {
        tx: Tx::Transfer {
            ts: Timestamp::from_secs(NOW.as_secs() - 86_400),
            pos: Position {
                asset: "USD".to_string(),
                qty: -1.0,
            },
        },
        ..entry("older", -1.0)
    };

    src_postgres::apply_entries(&db, ITEM, &[older, entry("newer", -2.0)], &[], &[])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db, user_id).await.unwrap();

    assert_eq!(entries[0].id, "newer");
    assert_eq!(entries[1].id, "older");
}

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn streams_round_trip(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let streams = vec![Stream {
        id: "stream-1".to_string(),
        label: "AUTOMATIC PAYMENT - THANK".to_string(),
        logo_url: None,
        category: Category::LoanPayments,
        amount: -2_078.50,
        frequency: "MONTHLY".to_string(),
        next_date: Some(NOW),
        is_active: true,
    }];

    src_postgres::save_streams(&db, ITEM, &streams)
        .await
        .unwrap();

    let back = src_postgres::load_streams(&db, user_id).await.unwrap();

    assert_eq!(back, streams);
}

/// The promise the app makes on the screen where it asks to connect a bank:
/// removing one takes its history with it.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn removing_a_bank_deletes_everything_that_came_through_it(db: PgPool) {
    let user_id = user(&db).await;
    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();
    src_postgres::apply_entries(&db, ITEM, &[entry("a", -25.0)], &[], &[])
        .await
        .unwrap();
    src_postgres::save_accounts(
        &db,
        ITEM,
        &[Account {
            id: "acc-1".to_string(),
            name: "Plaid Checking".to_string(),
            subtype: Some("checking".to_string()),
            mask: None,
            balance: 110.0,
            available: None,
            limit: None,
            apy: None,
            group: AccountGroup::Depository,
        }],
    )
    .await
    .unwrap();

    src_postgres::remove_item(&db, ITEM).await.unwrap();

    assert!(
        src_postgres::load_items(&db, &key(), user_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        src_postgres::load_entries(&db, user_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        src_postgres::load_accounts(&db, ITEM)
            .await
            .unwrap()
            .is_empty(),
        "the cascade takes the accounts too",
    );
}

/// The column does not hold the token.
///
/// The point of #21 in one assertion: whatever a `pg_dump`, a stolen volume or
/// a replica hands somebody, it is not the credential. Reads the raw bytes
/// rather than going through `load_items`, which would helpfully undo the very
/// thing being checked.
#[sqlx::test]
async fn the_access_token_is_not_in_the_column(db: PgPool) {
    let user_id = user(&db).await;

    src_postgres::save_item(&db, &key(), &item(user_id))
        .await
        .unwrap();

    let raw: Vec<u8> = sqlx::query_scalar("SELECT access_token FROM items WHERE item_id = $1")
        .bind(ITEM)
        .fetch_one(&db)
        .await
        .unwrap();

    let token = item(user_id).access_token;

    assert!(!raw.is_empty());
    assert_ne!(raw, token.as_bytes());

    // Not merely different — absent. A blob that still contained the token
    // somewhere inside it would pass the check above and fail the point.
    assert!(
        !raw.windows(token.len()).any(|w| w == token.as_bytes()),
        "the token is still in the column",
    );

    // And it comes back out for the server, which is the other half.
    let items = src_postgres::load_items(&db, &key(), user_id)
        .await
        .unwrap();

    assert_eq!(items[0].access_token, token);
}
