//! Who is paying, and until when.

use api_common::Timestamp;
use uuid::Uuid;

use crate::Db;
use crate::PostgresError;
use crate::postgres_ledger::seconds;
use crate::postgres_ledger::stamp;

/// Which rail proved a payment.
///
/// Mirrors the `billing_provider` enum in the database, so a provider that is
/// not one of these cannot be stored and cannot be read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "billing_provider", rename_all = "lowercase")]
pub enum BillingProvider {
    Stripe,
    Apple,
}

/// One user's subscription, as the database holds it.
#[derive(Debug, Clone)]
pub struct StoredSubscription {
    pub user_id: Uuid,
    pub provider: BillingProvider,
    pub customer_id: String,
    pub subscription_id: Option<String>,

    /// The provider's word for it: `active`, `past_due`, `canceled`. Not an
    /// enum — see the migration.
    pub status: String,

    /// The end of the period paid for. `None` has never been paid for.
    pub current_period_end: Option<Timestamp>,

    pub cancel_at_period_end: bool,

    /// The last event written from, so a repeat can be told from a new fact.
    pub last_event_id: Option<String>,
    pub last_event_at: Option<Timestamp>,
}

/// What one user's subscription is, if they have one.
pub async fn load_subscription(
    db: &Db,
    user_id: Uuid,
) -> Result<Option<StoredSubscription>, PostgresError> {
    let row = sqlx::query!(
        r#"SELECT user_id,
                  provider AS "provider: BillingProvider",
                  customer_id,
                  subscription_id,
                  status,
                  current_period_end,
                  cancel_at_period_end,
                  last_event_id,
                  last_event_at
           FROM subscriptions
           WHERE user_id = $1"#,
        user_id,
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| StoredSubscription {
        user_id: row.user_id,
        provider: row.provider,
        customer_id: row.customer_id,
        subscription_id: row.subscription_id,
        status: row.status,
        current_period_end: row.current_period_end.map(seconds),
        cancel_at_period_end: row.cancel_at_period_end,
        last_event_id: row.last_event_id,
        last_event_at: row.last_event_at.map(seconds),
    }))
}

/// Every subscription there is, for the read the server does at startup.
///
/// Entitlement is answered from what the server holds rather than by asking
/// the database on the request path, the same way the ledger is.
pub async fn load_subscriptions(db: &Db) -> Result<Vec<StoredSubscription>, PostgresError> {
    let rows = sqlx::query!(
        r#"SELECT user_id,
                  provider AS "provider: BillingProvider",
                  customer_id,
                  subscription_id,
                  status,
                  current_period_end,
                  cancel_at_period_end,
                  last_event_id,
                  last_event_at
           FROM subscriptions
           ORDER BY created_at"#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| StoredSubscription {
            user_id: row.user_id,
            provider: row.provider,
            customer_id: row.customer_id,
            subscription_id: row.subscription_id,
            status: row.status,
            current_period_end: row.current_period_end.map(seconds),
            cancel_at_period_end: row.cancel_at_period_end,
            last_event_id: row.last_event_id,
            last_event_at: row.last_event_at.map(seconds),
        })
        .collect())
}

/// Whose subscription this is, found by what the provider calls them.
///
/// A webhook names the customer and not the user, so this is the index from
/// one to the other.
pub async fn user_for_customer(
    db: &Db,
    provider: BillingProvider,
    customer_id: &str,
) -> Result<Option<Uuid>, PostgresError> {
    let row = sqlx::query!(
        r#"SELECT user_id FROM subscriptions
           WHERE provider = $1 AND customer_id = $2"#,
        provider as BillingProvider,
        customer_id,
    )
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| row.user_id))
}

/// Remember that this user is this provider's customer, before anything has
/// been paid.
///
/// Written when the customer is created rather than when the payment lands,
/// because the webhook that says it landed names only the customer — without
/// this row there would be nothing to look the user up by.
///
/// `status` starts as whatever the provider says a subscription with no
/// payment is, and `current_period_end` stays null, which is not entitlement.
pub async fn save_customer(
    db: &Db,
    user_id: Uuid,
    provider: BillingProvider,
    customer_id: &str,
    subscription_id: Option<&str>,
    status: &str,
) -> Result<(), PostgresError> {
    sqlx::query!(
        r#"INSERT INTO subscriptions
               (user_id, provider, customer_id, subscription_id, status)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (user_id) DO UPDATE SET
               provider        = EXCLUDED.provider,
               customer_id     = EXCLUDED.customer_id,
               subscription_id = EXCLUDED.subscription_id,
               status          = EXCLUDED.status,
               updated_at      = now()"#,
        user_id,
        provider as BillingProvider,
        customer_id,
        subscription_id,
        status,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Write what an event said about a subscription.
///
/// Keyed by the subscription rather than the user, because that is what the
/// event carries.
///
/// **The write is conditional, and that is the point.** Deliveries arrive more
/// than once and out of order — a provider retries until it is answered, and a
/// retry of an older event can land after a newer one — so an unconditional
/// write would let a stale status overwrite a fresh one. The `WHERE` takes the
/// event only if it is new (`last_event_id` differs) and not older than what
/// is already there.
///
/// Answers whether the row moved, so a caller can tell a fact from a repeat.
pub async fn apply_subscription_event(
    db: &Db,
    subscription_id: &str,
    status: &str,
    current_period_end: Option<Timestamp>,
    cancel_at_period_end: bool,
    event_id: &str,
    event_at: Timestamp,
) -> Result<bool, PostgresError> {
    let current_period_end = current_period_end.map(stamp);
    let event_at = stamp(event_at);

    let result = sqlx::query!(
        r#"UPDATE subscriptions SET
               status               = $2,
               current_period_end   = $3,
               cancel_at_period_end = $4,
               last_event_id        = $5,
               last_event_at        = $6,
               updated_at           = now()
           WHERE subscription_id = $1
             AND (last_event_id IS DISTINCT FROM $5)
             AND (last_event_at IS NULL OR last_event_at <= $6)"#,
        subscription_id,
        status,
        current_period_end,
        cancel_at_period_end,
        event_id,
        event_at,
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Tie a subscription id to a customer that did not have one.
///
/// The customer row is written first and the subscription is created against
/// it a moment later, so this is the second half of that.
pub async fn name_subscription(
    db: &Db,
    provider: BillingProvider,
    customer_id: &str,
    subscription_id: &str,
) -> Result<(), PostgresError> {
    sqlx::query!(
        r#"UPDATE subscriptions
           SET subscription_id = $3, updated_at = now()
           WHERE provider = $1 AND customer_id = $2"#,
        provider as BillingProvider,
        customer_id,
        subscription_id,
    )
    .execute(db)
    .await?;

    Ok(())
}
