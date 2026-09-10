//! Deleting an account, against a real Postgres.
//!
//! Deletion is the one operation where "it worked" has to be checked against
//! every table rather than believed. A row left behind is somebody's
//! transaction history surviving the request that was supposed to remove it,
//! and the reasons it must not are three — App Store guideline 5.1.1(v), the
//! deletion right in #16, and Plaid's questionnaire in #43.
//!
//! Most of the reach is `ON DELETE CASCADE` from `users`, and the tests below
//! are what says the cascades are actually wired: a schema change that drops
//! one is a leak nothing else here would notice.

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

const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

/// Thirty-two nothings. There is no secret here to keep.
fn key() -> src_postgres::DbKey {
    src_postgres::DbKey::parse("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
        .expect("thirty-two bytes is a key")
}

/// An account with a bank, a balance, a transaction and a stream — so a
/// deletion has something in every table to fail to remove.
async fn a_full_account(db: &PgPool, email: &str) -> Uuid {
    let user_id = src_postgres::upsert_user(db, email)
        .await
        .expect("the user is written");

    src_postgres::save_item(
        db,
        &key(),
        &StoredItem {
            item_id: ITEM.to_string(),
            user_id,
            access_token: "access-sandbox-1".to_string(),
            institution_id: Some("ins_109508".to_string()),
            institution_name: Some("First Platypus Bank".to_string()),
            cursor: None,
            error_code: None,
            synced_at: NOW,
        },
    )
    .await
    .expect("the item is written");

    src_postgres::save_accounts(
        db,
        ITEM,
        &[Account {
            id: "acc-1".to_string(),
            name: "Plaid Checking".to_string(),
            subtype: Some("checking".to_string()),
            mask: Some("0000".to_string()),
            balance: 110.0,
            available: Some(100.0),
            limit: None,
            apy: None,
            group: AccountGroup::Depository,
        }],
    )
    .await
    .expect("the accounts are written");

    src_postgres::apply_entries(
        db,
        ITEM,
        &[Entry {
            id: "entry-1".to_string(),
            tx: Tx::Transfer {
                ts: NOW,
                pos: Position {
                    asset: "USD".to_string(),
                    qty: -12.0,
                },
            },
            label: "Coffee".to_string(),
            account_id: Some("acc-1".to_string()),
            logo_url: None,
            category: Category::FoodAndDrink,
            pending: false,
            supersedes: None,
        }],
        &[],
        &[],
    )
    .await
    .expect("the entries are written");

    src_postgres::save_streams(
        db,
        ITEM,
        &[Stream {
            id: "stream-1".to_string(),
            label: "Rent".to_string(),
            logo_url: None,
            category: Category::RentAndUtilities,
            amount: -1_500.0,
            frequency: "MONTHLY".to_string(),
            next_date: Some(NOW),
            is_active: true,
        }],
    )
    .await
    .expect("the streams are written");

    src_postgres::save_customer(
        db,
        user_id,
        src_postgres::BillingProvider::Stripe,
        "cus_1",
        Some("sub_1"),
        "active",
    )
    .await
    .expect("the subscription is written");

    src_postgres::save_link(db, "link-1", user_id)
        .await
        .expect("the link session is written");

    src_postgres::create_session(
        db,
        &[1, 2, 3, 4],
        user_id,
        Some(Timestamp::from_secs(NOW.as_secs() + 3600)),
    )
    .await
    .expect("the session is written");

    src_postgres::save_code(
        db,
        email,
        &[5, 6, 7, 8],
        Timestamp::from_secs(NOW.as_secs() + 600),
    )
    .await
    .expect("the code is written");

    user_id
}

/// How many rows this user still has, table by table.
async fn what_is_left(db: &PgPool, user_id: Uuid, email: &str) -> Vec<(&'static str, i64)> {
    let one = |count: Option<i64>| count.unwrap_or_default();

    let users = sqlx::query!("SELECT count(*) FROM users WHERE user_id = $1", user_id)
        .fetch_one(db)
        .await
        .expect("the count works");

    let sessions = sqlx::query!("SELECT count(*) FROM sessions WHERE user_id = $1", user_id)
        .fetch_one(db)
        .await
        .expect("the count works");

    let identities = sqlx::query!(
        "SELECT count(*) FROM identities WHERE user_id = $1",
        user_id
    )
    .fetch_one(db)
    .await
    .expect("the count works");

    let items = sqlx::query!("SELECT count(*) FROM items WHERE user_id = $1", user_id)
        .fetch_one(db)
        .await
        .expect("the count works");

    let accounts = sqlx::query!("SELECT count(*) FROM accounts WHERE item_id = $1", ITEM)
        .fetch_one(db)
        .await
        .expect("the count works");

    let entries = sqlx::query!("SELECT count(*) FROM entries WHERE item_id = $1", ITEM)
        .fetch_one(db)
        .await
        .expect("the count works");

    let streams = sqlx::query!("SELECT count(*) FROM streams WHERE item_id = $1", ITEM)
        .fetch_one(db)
        .await
        .expect("the count works");

    let subscriptions = sqlx::query!(
        "SELECT count(*) FROM subscriptions WHERE user_id = $1",
        user_id
    )
    .fetch_one(db)
    .await
    .expect("the count works");

    let links = sqlx::query!(
        "SELECT count(*) FROM link_sessions WHERE user_id = $1",
        user_id
    )
    .fetch_one(db)
    .await
    .expect("the count works");

    let codes = sqlx::query!("SELECT count(*) FROM login_codes WHERE email = $1", email)
        .fetch_one(db)
        .await
        .expect("the count works");

    vec![
        ("users", one(users.count)),
        ("sessions", one(sessions.count)),
        ("identities", one(identities.count)),
        ("items", one(items.count)),
        ("accounts", one(accounts.count)),
        ("entries", one(entries.count)),
        ("streams", one(streams.count)),
        ("subscriptions", one(subscriptions.count)),
        ("link_sessions", one(links.count)),
        ("login_codes", one(codes.count)),
    ]
}

/// The claim: nothing is left, in any table.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn deleting_an_account_leaves_nothing_behind(db: PgPool) {
    let email = "gone@example.com";
    let user_id = a_full_account(&db, email).await;

    // Everything is there first, or the test below proves nothing.
    let before = what_is_left(&db, user_id, email).await;

    for (table, count) in &before {
        assert!(*count > 0, "{table} had nothing to delete");
    }

    src_postgres::delete_user(&db, user_id)
        .await
        .expect("the deletion works");

    let after = what_is_left(&db, user_id, email).await;

    for (table, count) in &after {
        assert_eq!(
            *count, 0,
            "{table} still has rows after the account was deleted"
        );
    }
}

/// The tokens come back before the rows go, because a token nobody holds is a
/// bank that bills forever.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn the_access_tokens_are_readable_before_the_rows_go(db: PgPool) {
    let user_id = a_full_account(&db, "gone@example.com").await;

    let tokens = src_postgres::load_access_tokens(&db, user_id, &key())
        .await
        .expect("the read works");

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, ITEM);
    assert_eq!(
        tokens[0].1, "access-sandbox-1",
        "the token is opened on the way out, or nothing can remove the item"
    );
}

/// And after deletion there is nothing left to read them from — which is why
/// the order matters.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn there_are_no_tokens_left_to_read_afterwards(db: PgPool) {
    let user_id = a_full_account(&db, "gone@example.com").await;

    src_postgres::delete_user(&db, user_id)
        .await
        .expect("the deletion works");

    let tokens = src_postgres::load_access_tokens(&db, user_id, &key())
        .await
        .expect("the read works");

    assert!(tokens.is_empty());
}

/// Deleting one account leaves another alone. The cascades key on the user, and
/// a `DELETE` that reached further would be the worst bug in the file.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn deleting_one_account_does_not_touch_another(db: PgPool) {
    let gone = a_full_account(&db, "gone@example.com").await;

    let stays = src_postgres::upsert_user(&db, "stays@example.com")
        .await
        .expect("the user is written");

    src_postgres::save_customer(
        &db,
        stays,
        src_postgres::BillingProvider::Stripe,
        "cus_2",
        Some("sub_2"),
        "active",
    )
    .await
    .expect("the subscription is written");

    src_postgres::delete_user(&db, gone)
        .await
        .expect("the deletion works");

    assert!(
        src_postgres::load_subscription(&db, stays)
            .await
            .expect("the read works")
            .is_some(),
        "the other account's subscription went with it",
    );
}

/// The address is free afterwards, and signing up again is a new account
/// rather than the old one coming back.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn the_address_can_sign_up_again_as_somebody_new(db: PgPool) {
    let email = "gone@example.com";
    let first = a_full_account(&db, email).await;

    src_postgres::delete_user(&db, first)
        .await
        .expect("the deletion works");

    let second = src_postgres::upsert_user(&db, email)
        .await
        .expect("the user is written");

    assert_ne!(
        first, second,
        "signing up again brought the deleted account back rather than making a new one",
    );

    // And it starts with nothing: no subscription, no banks.
    assert!(
        src_postgres::load_subscription(&db, second)
            .await
            .expect("the read works")
            .is_none()
    );
}

/// Deleting an account that is not there is not a failure. A retried request,
/// or two devices asking at once, must not turn into a 500.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn deleting_an_account_twice_is_not_an_error(db: PgPool) {
    let user_id = a_full_account(&db, "gone@example.com").await;

    src_postgres::delete_user(&db, user_id)
        .await
        .expect("the deletion works");

    src_postgres::delete_user(&db, user_id)
        .await
        .expect("deleting nothing is not an error");
}
