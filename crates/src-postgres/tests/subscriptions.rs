//! The subscription queries, against a real Postgres.
//!
//! Most of these are about one thing: a webhook is not delivered once. A
//! provider retries until it is answered, and a retry of an older event can
//! land after a newer one — so the write is conditional, and what it refuses
//! matters as much as what it takes. Getting that wrong bills somebody
//! correctly and then locks them out, which is the failure nobody reports
//! because they have already left.

use api_common::Timestamp;
use sqlx::PgPool;
use src_postgres::BillingProvider;
use uuid::Uuid;

const CUSTOMER: &str = "cus_test";
const SUBSCRIPTION: &str = "sub_test";

/// A moment to hang the events off.
fn at(secs: u64) -> Timestamp {
    Timestamp::from_secs(secs)
}

/// A user to own the subscription, since the row is keyed by one.
async fn a_user(db: &PgPool) -> Uuid {
    src_postgres::upsert_user(db, "someone@example.com")
        .await
        .expect("the user is written")
}

/// The customer row exists before anything is paid for, and is not
/// entitlement: no period has been paid for yet.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_customer_is_remembered_before_a_payment(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        Some(SUBSCRIPTION),
        "incomplete",
    )
    .await
    .expect("the customer is written");

    let stored = src_postgres::load_subscription(&db, user_id)
        .await
        .expect("the read works")
        .expect("there is a row");

    assert_eq!(stored.provider, BillingProvider::Stripe);
    assert_eq!(stored.customer_id, CUSTOMER);
    assert_eq!(stored.status, "incomplete");
    assert_eq!(stored.current_period_end, None);
    assert!(!stored.cancel_at_period_end);
}

/// A webhook names the customer, not the user. This is the index between them.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_customer_says_whose_it_is(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        Some(SUBSCRIPTION),
        "incomplete",
    )
    .await
    .expect("the customer is written");

    let found = src_postgres::user_for_customer(&db, BillingProvider::Stripe, CUSTOMER)
        .await
        .expect("the read works");

    assert_eq!(found, Some(user_id));

    // The same id under the other provider is a different customer.
    let apple = src_postgres::user_for_customer(&db, BillingProvider::Apple, CUSTOMER)
        .await
        .expect("the read works");

    assert_eq!(apple, None);
}

/// The ordinary case: an event arrives and the row moves.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn an_event_writes_what_it_says(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        Some(SUBSCRIPTION),
        "incomplete",
    )
    .await
    .expect("the customer is written");

    let took = src_postgres::apply_subscription_event(
        &db,
        SUBSCRIPTION,
        "active",
        Some(at(2_000)),
        false,
        "evt_1",
        at(1_000),
    )
    .await
    .expect("the write works");

    assert!(took, "a new event moves the row");

    let stored = src_postgres::load_subscription(&db, user_id)
        .await
        .expect("the read works")
        .expect("there is a row");

    assert_eq!(stored.status, "active");
    assert_eq!(stored.current_period_end.map(|t| t.as_secs()), Some(2_000));
    assert_eq!(stored.last_event_id.as_deref(), Some("evt_1"));
}

/// The same delivery twice. The second changes nothing and says so, which is
/// what lets a caller tell a fact from a repeat.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn the_same_event_twice_is_written_once(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        Some(SUBSCRIPTION),
        "incomplete",
    )
    .await
    .expect("the customer is written");

    let first = src_postgres::apply_subscription_event(
        &db,
        SUBSCRIPTION,
        "active",
        Some(at(2_000)),
        false,
        "evt_1",
        at(1_000),
    )
    .await
    .expect("the write works");

    let again = src_postgres::apply_subscription_event(
        &db,
        SUBSCRIPTION,
        "active",
        Some(at(2_000)),
        false,
        "evt_1",
        at(1_000),
    )
    .await
    .expect("the write works");

    assert!(first);
    assert!(!again, "the same event id is a repeat, not a fact");
}

/// The one that matters. A retry of an older event lands after a newer one,
/// and must not undo it — a `canceled` arriving late over an `active` is
/// somebody locked out of what they have paid for.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn an_older_event_arriving_late_does_not_win(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        Some(SUBSCRIPTION),
        "incomplete",
    )
    .await
    .expect("the customer is written");

    // The newer fact lands first.
    src_postgres::apply_subscription_event(
        &db,
        SUBSCRIPTION,
        "active",
        Some(at(9_000)),
        false,
        "evt_new",
        at(5_000),
    )
    .await
    .expect("the write works");

    // A retry of something Stripe made earlier arrives after it.
    let took = src_postgres::apply_subscription_event(
        &db,
        SUBSCRIPTION,
        "canceled",
        Some(at(1_000)),
        true,
        "evt_old",
        at(1_000),
    )
    .await
    .expect("the write works");

    assert!(!took, "an older event is refused");

    let stored = src_postgres::load_subscription(&db, user_id)
        .await
        .expect("the read works")
        .expect("there is a row");

    assert_eq!(stored.status, "active", "the newer fact stands");
    assert_eq!(stored.current_period_end.map(|t| t.as_secs()), Some(9_000));
}

/// Cancelling at period end is not the end of access, and the row says both
/// things separately: the flag for what to draw, the date for what to allow.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_cancellation_keeps_the_period_it_paid_for(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        Some(SUBSCRIPTION),
        "active",
    )
    .await
    .expect("the customer is written");

    src_postgres::apply_subscription_event(
        &db,
        SUBSCRIPTION,
        "active",
        Some(at(9_000)),
        true,
        "evt_1",
        at(1_000),
    )
    .await
    .expect("the write works");

    let stored = src_postgres::load_subscription(&db, user_id)
        .await
        .expect("the read works")
        .expect("there is a row");

    assert!(stored.cancel_at_period_end);
    assert_eq!(stored.status, "active");
    assert_eq!(stored.current_period_end.map(|t| t.as_secs()), Some(9_000));
}

/// An event about a subscription nobody has a row for is not an error, and
/// writes nothing. It happens: a customer created and abandoned, or an object
/// from before this table existed.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn an_event_for_an_unknown_subscription_writes_nothing(db: PgPool) {
    let took = src_postgres::apply_subscription_event(
        &db,
        "sub_nobody_has",
        "active",
        Some(at(2_000)),
        false,
        "evt_1",
        at(1_000),
    )
    .await
    .expect("the write works");

    assert!(!took);
}

/// The subscription id arrives a moment after the customer does.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_subscription_can_be_named_after_the_customer(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        None,
        "incomplete",
    )
    .await
    .expect("the customer is written");

    src_postgres::name_subscription(&db, BillingProvider::Stripe, CUSTOMER, SUBSCRIPTION)
        .await
        .expect("the write works");

    let stored = src_postgres::load_subscription(&db, user_id)
        .await
        .expect("the read works")
        .expect("there is a row");

    assert_eq!(stored.subscription_id.as_deref(), Some(SUBSCRIPTION));
}

/// The startup read, which is where entitlement comes from.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn every_subscription_comes_back(db: PgPool) {
    let one = src_postgres::upsert_user(&db, "one@example.com")
        .await
        .expect("the user is written");

    let two = src_postgres::upsert_user(&db, "two@example.com")
        .await
        .expect("the user is written");

    src_postgres::save_customer(&db, one, BillingProvider::Stripe, "cus_1", None, "active")
        .await
        .expect("the customer is written");

    src_postgres::save_customer(&db, two, BillingProvider::Apple, "orig_2", None, "active")
        .await
        .expect("the customer is written");

    let all = src_postgres::load_subscriptions(&db)
        .await
        .expect("the read works");

    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|s| s.provider == BillingProvider::Stripe));
    assert!(all.iter().any(|s| s.provider == BillingProvider::Apple));
}

/// A user with no row has never paid, and that is a `None` rather than a
/// failure.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_user_who_has_never_paid_has_no_row(db: PgPool) {
    let user_id = a_user(&db).await;

    let stored = src_postgres::load_subscription(&db, user_id)
        .await
        .expect("the read works");

    assert!(stored.is_none());
}

/// Deleting the account takes the billing row with it. Issue #64 is the
/// feature; this is the half the schema owes it.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn deleting_a_user_takes_their_subscription(db: PgPool) {
    let user_id = a_user(&db).await;

    src_postgres::save_customer(
        &db,
        user_id,
        BillingProvider::Stripe,
        CUSTOMER,
        Some(SUBSCRIPTION),
        "active",
    )
    .await
    .expect("the customer is written");

    sqlx::query!("DELETE FROM users WHERE user_id = $1", user_id)
        .execute(&db)
        .await
        .expect("the user is deleted");

    let stored = src_postgres::load_subscription(&db, user_id)
        .await
        .expect("the read works");

    assert!(stored.is_none());
}
