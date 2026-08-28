//! The queries, against a real Postgres.
//!
//! Ignored by default — `cargo test` has to pass on a clean checkout with no
//! Docker, the same reason the server treats `DATABASE_URL` as optional:
//!
//! ```sh
//! ./scripts/db-init.sh
//! cargo test -p src-postgres -- --ignored
//! ```
//!
//! These read and write the same tables, and `load_entries` is deliberately
//! global — the server wants the whole ledger — so they take a lock and start
//! from an empty database rather than depending on each other's leftovers.

use std::sync::OnceLock;

use api_common::Timestamp;
use api_ledger::Account;
use api_ledger::AccountGroup;
use api_ledger::Category;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Stream;
use api_ledger::Tx;
use src_postgres::Db;
use src_postgres::StoredItem;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;

const DEFAULT_URL: &str = "postgres://meridian:meridian@localhost:5432/meridian";

const ITEM: &str = "item-test";

/// 2026-08-19, the day the fixtures are drawn against.
const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

/// One test at a time, on an empty database.
///
/// The guard is returned rather than dropped so it is held for the whole test:
/// two of these interleaving would each see the other's rows through
/// `load_entries`, and the failure would depend on scheduling.
async fn alone() -> (MutexGuard<'static, ()>, Db) {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    let guard = LOCK.get_or_init(|| Mutex::new(())).lock().await;

    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());

    let db = src_postgres::connect(&url)
        .await
        .expect("postgres should be up — ./scripts/db-init.sh");

    src_postgres::migrate(&db).await.expect("migrations run");

    // Items cascade, so this clears everything.
    sqlx::query("DELETE FROM items")
        .execute(&db)
        .await
        .expect("a clean slate");

    (guard, db)
}

fn item() -> StoredItem {
    StoredItem {
        item_id: ITEM.to_string(),
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

#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn an_item_round_trips() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

    let items = src_postgres::load_items(&db).await.unwrap();

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
#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn saving_an_item_again_does_not_rewind_the_cursor() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();
    src_postgres::save_cursor(&db, ITEM, "cursor-abc")
        .await
        .unwrap();

    src_postgres::save_item(&db, &item()).await.unwrap();

    let items = src_postgres::load_items(&db).await.unwrap();

    assert_eq!(items[0].cursor.as_deref(), Some("cursor-abc"));
}

#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn accounts_round_trip_with_their_money_intact() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

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
#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn applying_the_same_entries_twice_stores_them_once() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

    let added = vec![entry("a", -25.0), entry("b", -4.33)];

    src_postgres::apply_entries(&db, ITEM, &added, &[], &[])
        .await
        .unwrap();
    src_postgres::apply_entries(&db, ITEM, &added, &[], &[])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db).await.unwrap();

    assert_eq!(entries.len(), 2, "a transaction appears once");
    assert_eq!(entries.iter().filter(|e| e.id == "a").count(), 1);
}

#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn a_modified_entry_replaces_its_old_self() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

    src_postgres::apply_entries(&db, ITEM, &[entry("a", -25.0)], &[], &[])
        .await
        .unwrap();

    let mut changed = entry("a", -30.0);
    changed.label = "Uber Eats".to_string();

    src_postgres::apply_entries(&db, ITEM, &[], &[changed], &[])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db).await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].amount(), -30.0);
    assert_eq!(entries[0].label, "Uber Eats");
}

/// Trap two from docs/PLAID.md. A posted transaction arrives with a new id
/// pointing at the pending one it replaces, and unless the pending one goes
/// every purchase is counted twice for the days between the two.
#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn a_posted_entry_drops_the_pending_one_it_replaces() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

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

    let entries = src_postgres::load_entries(&db).await.unwrap();

    assert_eq!(entries.len(), 1, "counted once, not twice");
    assert_eq!(entries[0].id, "posted-1");
}

#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn a_retracted_entry_is_removed() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

    src_postgres::apply_entries(&db, ITEM, &[entry("a", -25.0), entry("b", -4.33)], &[], &[])
        .await
        .unwrap();

    src_postgres::apply_entries(&db, ITEM, &[], &[], &["a".to_string()])
        .await
        .unwrap();

    let entries = src_postgres::load_entries(&db).await.unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "b");
}

/// Newest first, which is the order every screen shows.
#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn entries_come_back_newest_first() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

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

    let entries = src_postgres::load_entries(&db).await.unwrap();

    assert_eq!(entries[0].id, "newer");
    assert_eq!(entries[1].id, "older");
}

#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn streams_round_trip() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();

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

    let back = src_postgres::load_streams(&db).await.unwrap();

    assert_eq!(back, streams);
}

/// The promise the app makes on the screen where it asks to connect a bank:
/// removing one takes its history with it.
#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn removing_a_bank_deletes_everything_that_came_through_it() {
    let (_guard, db) = alone().await;

    src_postgres::save_item(&db, &item()).await.unwrap();
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

    assert!(src_postgres::load_items(&db).await.unwrap().is_empty());
    assert!(src_postgres::load_entries(&db).await.unwrap().is_empty());
    assert!(
        src_postgres::load_accounts(&db, ITEM)
            .await
            .unwrap()
            .is_empty(),
        "the cascade takes the accounts too",
    );
}
