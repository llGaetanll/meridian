//! The ledger: accounts, transactions and recurring streams.
//!
//! Every statement that touches them is here, and the shapes that go in and
//! out are `api-ledger`'s. Nothing in this file decides anything — which
//! account is a debt, what a subtype is called, whether a stream is worth
//! counting — those are all answered elsewhere, from the raw values these
//! functions carry.

use api_common::Timestamp;
use api_ledger::Account;
use api_ledger::AccountGroup;
use api_ledger::Category;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Stream;
use api_ledger::Tx;
use chrono::DateTime;
use chrono::Utc;
use sqlx::QueryBuilder;
use sqlx::Row;

use crate::Db;
use crate::PostgresError;
use crate::postgres_money::from_minor;
use crate::postgres_money::to_minor;

/// How many rows go into one statement.
///
/// Postgres takes at most 65535 parameters per statement, and the widest table
/// here binds eleven per row. A thousand leaves room to add a column without
/// anybody rediscovering that limit through a first sync failing.
const CHUNK: usize = 1_000;

/// A timestamp as Postgres holds it.
///
/// The epoch is out of range only for values no bank feed produces, and a date
/// that cannot be represented is stored as the epoch rather than failing a
/// whole sync over one row.
pub(crate) fn stamp(ts: Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(ts.as_secs() as i64, 0).unwrap_or_else(|| {
        tracing::warn!(seconds = ts.as_secs(), "a timestamp fell outside postgres");

        DateTime::UNIX_EPOCH
    })
}

/// And back.
pub(crate) fn seconds(at: DateTime<Utc>) -> Timestamp {
    Timestamp::from_secs(at.timestamp().max(0) as u64)
}

/// Replace an Item's accounts with what the bank just said.
///
/// Wholesale rather than a diff, because `/accounts/balance/get` answers with
/// every account every time — there is no "changed since" to apply. In one
/// transaction, so nothing ever reads a connection with half its accounts.
pub async fn save_accounts(
    db: &Db,
    item_id: &str,
    accounts: &[Account],
) -> Result<(), PostgresError> {
    let mut tx = db.begin().await?;

    sqlx::query("DELETE FROM accounts WHERE item_id = $1")
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

    for chunk in accounts.chunks(CHUNK) {
        let mut query = QueryBuilder::new(
            "INSERT INTO accounts (account_id, item_id, name, subtype, account_group, mask, \
             balance_minor, available_minor, limit_minor, apy) ",
        );

        query.push_values(chunk, |mut row, account| {
            row.push_bind(&account.id)
                .push_bind(item_id)
                .push_bind(&account.name)
                .push_bind(&account.subtype)
                .push_bind(account.group.as_plaid())
                .push_bind(&account.mask)
                .push_bind(to_minor(account.balance))
                .push_bind(account.available.map(to_minor))
                .push_bind(account.limit.map(to_minor))
                .push_bind(account.apy);
        });

        query.build().execute(&mut *tx).await?;
    }

    tx.commit().await?;

    Ok(())
}

/// An Item's accounts.
pub async fn load_accounts(db: &Db, item_id: &str) -> Result<Vec<Account>, PostgresError> {
    let rows = sqlx::query(
        "SELECT account_id, name, subtype, account_group, mask, balance_minor, \
         available_minor, limit_minor, apy
         FROM accounts WHERE item_id = $1 ORDER BY account_id",
    )
    .bind(item_id)
    .fetch_all(db)
    .await?;

    rows.iter()
        .map(|row| {
            let group: String = row.try_get("account_group")?;

            Ok(Account {
                id: row.try_get("account_id")?,
                name: row.try_get("name")?,
                subtype: row.try_get("subtype")?,
                mask: row.try_get("mask")?,
                balance: from_minor(row.try_get("balance_minor")?),
                available: row
                    .try_get::<Option<i64>, _>("available_minor")?
                    .map(from_minor),
                limit: row
                    .try_get::<Option<i64>, _>("limit_minor")?
                    .map(from_minor),
                apy: row.try_get("apy")?,
                group: AccountGroup::from_plaid(&group),
            })
        })
        .collect()
}

/// Apply one sync's worth of changes.
///
/// The counterpart of [`api_ledger::Ledger::apply`], in the same order and for
/// the same reasons — a diff, never a snapshot. A second sync a minute later
/// legitimately returns three empty lists, and assigning that over what is
/// held is how the app went blank on refresh (`docs/PLAID.md`).
///
/// Three steps, in one transaction so a failure part-way leaves the ledger as
/// it was rather than half-updated:
///
/// 1. Drop what the bank retracted.
/// 2. Upsert everything added and modified. **The upsert is the point**: an
///    entry the ledger already holds replaces itself instead of joining the
///    list again, which is exactly the bug that made one transaction appear
///    three times.
/// 3. Drop the pending entries that have since posted.
pub async fn apply_entries(
    db: &Db,
    item_id: &str,
    added: &[Entry],
    modified: &[Entry],
    removed: &[String],
) -> Result<(), PostgresError> {
    let mut tx = db.begin().await?;

    if !removed.is_empty() {
        sqlx::query("DELETE FROM entries WHERE id = ANY($1)")
            .bind(removed)
            .execute(&mut *tx)
            .await?;
    }

    let mut skipped = 0;

    for chunk in modified
        .iter()
        .chain(added)
        .collect::<Vec<_>>()
        .chunks(CHUNK)
    {
        let writable: Vec<&&Entry> = chunk.iter().filter(|e| transfer(e).is_some()).collect();

        skipped += chunk.len() - writable.len();

        if writable.is_empty() {
            continue;
        }

        let mut query = QueryBuilder::new(
            "INSERT INTO entries (id, item_id, account_id, ts, currency, amount_minor, \
             label, category, logo_url, pending, supersedes) ",
        );

        query.push_values(writable, |mut row, entry| {
            // Checked by the filter above, so the fallback is unreachable
            // rather than a guess.
            let (currency, amount) = transfer(entry).unwrap_or(("", 0.0));

            row.push_bind(&entry.id)
                .push_bind(item_id)
                .push_bind(&entry.account_id)
                .push_bind(stamp(entry.ts()))
                .push_bind(currency.to_string())
                .push_bind(to_minor(amount))
                .push_bind(&entry.label)
                .push_bind(entry.category.as_plaid())
                .push_bind(&entry.logo_url)
                .push_bind(entry.pending)
                .push_bind(&entry.supersedes);
        });

        query.push(
            " ON CONFLICT (id) DO UPDATE SET
                 item_id      = EXCLUDED.item_id,
                 account_id   = EXCLUDED.account_id,
                 ts           = EXCLUDED.ts,
                 currency     = EXCLUDED.currency,
                 amount_minor = EXCLUDED.amount_minor,
                 label        = EXCLUDED.label,
                 category     = EXCLUDED.category,
                 logo_url     = EXCLUDED.logo_url,
                 pending      = EXCLUDED.pending,
                 supersedes   = EXCLUDED.supersedes",
        );

        query.build().execute(&mut *tx).await?;
    }

    // Plaid never updates a pending transaction in place: a posted one arrives
    // with its own id pointing back at the old. Unless the old one goes, every
    // purchase is counted twice for the days between authorisation and
    // settlement, and the numbers stay plausible throughout.
    sqlx::query(
        "DELETE FROM entries WHERE id IN
             (SELECT supersedes FROM entries WHERE supersedes IS NOT NULL)",
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    if skipped > 0 {
        // Only a brokerage feed produces these, and none is wired up. The line
        // is here so that the day one is, this is a message rather than a
        // mystery: the schema holds one movement per row and a trade has two.
        tracing::warn!(skipped, "entries that are not transfers were not stored");
    }

    Ok(())
}

/// The whole ledger, newest first — the order every screen shows.
///
/// Ties break on id so the order is total: two transactions on the same day
/// would otherwise come back differently run to run, and a list that reshuffles
/// on refresh looks broken.
pub async fn load_entries(db: &Db) -> Result<Vec<Entry>, PostgresError> {
    let rows = sqlx::query(
        "SELECT id, account_id, ts, currency, amount_minor, label, category, \
         logo_url, pending, supersedes
         FROM entries ORDER BY ts DESC, id ASC",
    )
    .fetch_all(db)
    .await?;

    rows.iter()
        .map(|row| {
            let ts: DateTime<Utc> = row.try_get("ts")?;
            let category: String = row.try_get("category")?;

            Ok(Entry {
                id: row.try_get("id")?,
                tx: Tx::Transfer {
                    ts: seconds(ts),
                    pos: Position {
                        asset: row.try_get("currency")?,
                        qty: from_minor(row.try_get("amount_minor")?),
                    },
                },
                label: row.try_get("label")?,
                account_id: row.try_get("account_id")?,
                logo_url: row.try_get("logo_url")?,
                category: Category::from_plaid(&category),
                pending: row.try_get("pending")?,
                supersedes: row.try_get("supersedes")?,
            })
        })
        .collect()
}

/// Replace an Item's recurring streams.
///
/// Wholesale, like the accounts: `/transactions/recurring/get` re-derives every
/// stream from the whole history each time, so there is no diff to apply.
pub async fn save_streams(db: &Db, item_id: &str, streams: &[Stream]) -> Result<(), PostgresError> {
    let mut tx = db.begin().await?;

    sqlx::query("DELETE FROM streams WHERE item_id = $1")
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

    for chunk in streams.chunks(CHUNK) {
        let mut query = QueryBuilder::new(
            "INSERT INTO streams (id, item_id, label, category, amount_minor, \
             frequency, next_date) ",
        );

        query.push_values(chunk, |mut row, stream| {
            row.push_bind(&stream.id)
                .push_bind(item_id)
                .push_bind(&stream.label)
                .push_bind(stream.category.as_plaid())
                .push_bind(to_minor(stream.amount))
                .push_bind(&stream.frequency)
                .push_bind(stream.next_date.map(stamp));
        });

        query.build().execute(&mut *tx).await?;
    }

    tx.commit().await?;

    Ok(())
}

/// Every stream. All of them are active: an inactive one is dropped before it
/// ever reaches this crate, so nothing here has to decide.
pub async fn load_streams(db: &Db) -> Result<Vec<Stream>, PostgresError> {
    let rows = sqlx::query(
        "SELECT id, label, category, amount_minor, frequency, next_date
         FROM streams ORDER BY id",
    )
    .fetch_all(db)
    .await?;

    rows.iter()
        .map(|row| {
            let category: String = row.try_get("category")?;

            Ok(Stream {
                id: row.try_get("id")?,
                label: row.try_get("label")?,
                logo_url: None,
                category: Category::from_plaid(&category),
                amount: from_minor(row.try_get("amount_minor")?),
                frequency: row.try_get("frequency")?,
                next_date: row
                    .try_get::<Option<DateTime<Utc>>, _>("next_date")?
                    .map(seconds),
                is_active: true,
            })
        })
        .collect()
}

/// The currency and amount of an entry, when it is a movement of one thing.
///
/// `None` for a trade, which is two positions and does not fit a row that
/// holds one. Nothing produces trades today; see the warning in
/// [`apply_entries`].
fn transfer(entry: &Entry) -> Option<(&str, f64)> {
    match &entry.tx {
        Tx::Transfer { pos, .. } => Some((&pos.asset, pos.qty)),
        Tx::Trade { .. } => None,
    }
}
