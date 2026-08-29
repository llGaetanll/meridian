//! The ledger: accounts, transactions and recurring streams.
//!
//! Every statement that touches them is here, and the shapes that go in and
//! out are `api-ledger`'s. Nothing in this file decides anything — which
//! account is a debt, what a subtype is called, whether a stream is worth
//! counting — those are answered elsewhere, from the raw values these
//! functions carry.
//!
//! The bulk inserts go through `UNNEST` rather than a builder that pastes one
//! `VALUES` clause per row. Two reasons, and the second is the real one: it is
//! a single statement whatever the row count, so a first sync of ten thousand
//! transactions is one round trip and there is no parameter ceiling to chunk
//! around; and it is *static* SQL, so the compiler can check it against the
//! schema like everything else here.

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
use uuid::Uuid;

use crate::Db;
use crate::PostgresError;
use crate::postgres_money::from_minor;
use crate::postgres_money::to_minor;

/// A timestamp as Postgres holds it.
///
/// A date that cannot be represented is stored as the epoch rather than
/// failing a whole sync over one row. Nothing a bank feed produces is
/// anywhere near the edge.
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
    let ids: Vec<String> = accounts.iter().map(|a| a.id.clone()).collect();
    let names: Vec<String> = accounts.iter().map(|a| a.name.clone()).collect();
    let subtypes: Vec<Option<String>> = accounts.iter().map(|a| a.subtype.clone()).collect();
    let groups: Vec<String> = accounts
        .iter()
        .map(|a| a.group.as_plaid().to_string())
        .collect();
    let masks: Vec<Option<String>> = accounts.iter().map(|a| a.mask.clone()).collect();
    let balances: Vec<i64> = accounts.iter().map(|a| to_minor(a.balance)).collect();
    let availables: Vec<Option<i64>> = accounts.iter().map(|a| a.available.map(to_minor)).collect();
    let limits: Vec<Option<i64>> = accounts.iter().map(|a| a.limit.map(to_minor)).collect();
    let apys: Vec<Option<f64>> = accounts.iter().map(|a| a.apy).collect();

    let mut tx = db.begin().await?;

    sqlx::query!("DELETE FROM accounts WHERE item_id = $1", item_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "INSERT INTO accounts (account_id, item_id, name, subtype, account_group, mask,
                               balance_minor, available_minor, limit_minor, apy)
         SELECT account_id, $1, name, subtype, account_group, mask,
                balance_minor, available_minor, limit_minor, apy
         FROM UNNEST($2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                     $7::int8[], $8::int8[], $9::int8[], $10::float8[])
              AS t(account_id, name, subtype, account_group, mask,
                   balance_minor, available_minor, limit_minor, apy)",
        item_id,
        &ids,
        &names,
        &subtypes as &[Option<String>],
        &groups,
        &masks as &[Option<String>],
        &balances,
        &availables as &[Option<i64>],
        &limits as &[Option<i64>],
        &apys as &[Option<f64>],
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// An Item's accounts.
pub async fn load_accounts(db: &Db, item_id: &str) -> Result<Vec<Account>, PostgresError> {
    let rows = sqlx::query!(
        "SELECT account_id, name, subtype, account_group, mask, balance_minor,
                available_minor, limit_minor, apy
         FROM accounts WHERE item_id = $1 ORDER BY account_id",
        item_id,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Account {
            id: row.account_id,
            name: row.name,
            subtype: row.subtype,
            mask: row.mask,
            balance: from_minor(row.balance_minor),
            available: row.available_minor.map(from_minor),
            limit: row.limit_minor.map(from_minor),
            apy: row.apy,
            group: AccountGroup::from_plaid(&row.account_group),
        })
        .collect())
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
    // A trade is two positions and a row holds one movement, so it has no
    // shape here. Nothing produces trades today; the count is reported below
    // so that the day something does, this is a message rather than a mystery.
    let writable: Vec<&Entry> = modified
        .iter()
        .chain(added)
        .filter(|entry| transfer(entry).is_some())
        .collect();

    let skipped = modified.len() + added.len() - writable.len();

    let ids: Vec<String> = writable.iter().map(|e| e.id.clone()).collect();
    let account_ids: Vec<Option<String>> = writable.iter().map(|e| e.account_id.clone()).collect();
    let timestamps: Vec<DateTime<Utc>> = writable.iter().map(|e| stamp(e.ts())).collect();
    let currencies: Vec<String> = writable
        .iter()
        .map(|e| transfer(e).map(|(c, _)| c.to_string()).unwrap_or_default())
        .collect();
    let amounts: Vec<i64> = writable
        .iter()
        .map(|e| to_minor(transfer(e).map(|(_, a)| a).unwrap_or_default()))
        .collect();
    let labels: Vec<String> = writable.iter().map(|e| e.label.clone()).collect();
    let categories: Vec<String> = writable
        .iter()
        .map(|e| e.category.as_plaid().to_string())
        .collect();
    let logos: Vec<Option<String>> = writable.iter().map(|e| e.logo_url.clone()).collect();
    let pendings: Vec<bool> = writable.iter().map(|e| e.pending).collect();
    let supersedes: Vec<Option<String>> = writable.iter().map(|e| e.supersedes.clone()).collect();

    let mut tx = db.begin().await?;

    sqlx::query!("DELETE FROM entries WHERE id = ANY($1)", removed)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "INSERT INTO entries (id, item_id, account_id, ts, currency, amount_minor,
                              label, category, logo_url, pending, supersedes)
         SELECT id, $1, account_id, ts, currency, amount_minor,
                label, category, logo_url, pending, supersedes
         FROM UNNEST($2::text[], $3::text[], $4::timestamptz[], $5::text[], $6::int8[],
                     $7::text[], $8::text[], $9::text[], $10::bool[], $11::text[])
              AS t(id, account_id, ts, currency, amount_minor,
                   label, category, logo_url, pending, supersedes)
         ON CONFLICT (id) DO UPDATE SET
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
        item_id,
        &ids,
        &account_ids as &[Option<String>],
        &timestamps,
        &currencies,
        &amounts,
        &labels,
        &categories,
        &logos as &[Option<String>],
        &pendings,
        &supersedes as &[Option<String>],
    )
    .execute(&mut *tx)
    .await?;

    // The source never updates a pending transaction in place: a posted one
    // arrives with its own id pointing back at the old. Unless the old one
    // goes, every purchase is counted twice for the days between authorisation
    // and settlement, and the numbers stay plausible throughout.
    sqlx::query!(
        "DELETE FROM entries WHERE id IN
             (SELECT supersedes FROM entries WHERE supersedes IS NOT NULL)"
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    if skipped > 0 {
        tracing::warn!(skipped, "entries that are not transfers were not stored");
    }

    Ok(())
}

/// The whole ledger, newest first — the order every screen shows.
///
/// Ties break on id so the order is total: two transactions on the same day
/// would otherwise come back differently run to run, and a list that
/// reshuffles on refresh looks broken.
pub async fn load_entries(db: &Db, user_id: Uuid) -> Result<Vec<Entry>, PostgresError> {
    let rows = sqlx::query!(
        "SELECT e.id, e.account_id, e.ts, e.currency, e.amount_minor, e.label,
                e.category, e.logo_url, e.pending, e.supersedes
         FROM entries e
         JOIN items i USING (item_id)
         WHERE i.user_id = $1
         ORDER BY e.ts DESC, e.id ASC",
        user_id,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Entry {
            id: row.id,
            tx: Tx::Transfer {
                ts: seconds(row.ts),
                pos: Position {
                    asset: row.currency,
                    qty: from_minor(row.amount_minor),
                },
            },
            label: row.label,
            account_id: row.account_id,
            logo_url: row.logo_url,
            category: Category::from_plaid(&row.category),
            pending: row.pending,
            supersedes: row.supersedes,
        })
        .collect())
}

/// Replace an Item's recurring streams.
///
/// Wholesale, like the accounts: `/transactions/recurring/get` re-derives every
/// stream from the whole history each time, so there is no diff to apply.
pub async fn save_streams(db: &Db, item_id: &str, streams: &[Stream]) -> Result<(), PostgresError> {
    let ids: Vec<String> = streams.iter().map(|s| s.id.clone()).collect();
    let labels: Vec<String> = streams.iter().map(|s| s.label.clone()).collect();
    let categories: Vec<String> = streams
        .iter()
        .map(|s| s.category.as_plaid().to_string())
        .collect();
    let amounts: Vec<i64> = streams.iter().map(|s| to_minor(s.amount)).collect();
    let frequencies: Vec<String> = streams.iter().map(|s| s.frequency.clone()).collect();
    let next_dates: Vec<Option<DateTime<Utc>>> =
        streams.iter().map(|s| s.next_date.map(stamp)).collect();

    let mut tx = db.begin().await?;

    sqlx::query!("DELETE FROM streams WHERE item_id = $1", item_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "INSERT INTO streams (id, item_id, label, category, amount_minor, frequency, next_date)
         SELECT id, $1, label, category, amount_minor, frequency, next_date
         FROM UNNEST($2::text[], $3::text[], $4::text[], $5::int8[], $6::text[],
                     $7::timestamptz[])
              AS t(id, label, category, amount_minor, frequency, next_date)",
        item_id,
        &ids,
        &labels,
        &categories,
        &amounts,
        &frequencies,
        &next_dates as &[Option<DateTime<Utc>>],
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Every stream. All of them are active: an inactive one is dropped before it
/// ever reaches this crate, so nothing here has to decide.
pub async fn load_streams(db: &Db, user_id: Uuid) -> Result<Vec<Stream>, PostgresError> {
    let rows = sqlx::query!(
        "SELECT s.id, s.label, s.category, s.amount_minor, s.frequency, s.next_date
         FROM streams s
         JOIN items i USING (item_id)
         WHERE i.user_id = $1
         ORDER BY s.id",
        user_id,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Stream {
            id: row.id,
            label: row.label,
            logo_url: None,
            category: Category::from_plaid(&row.category),
            amount: from_minor(row.amount_minor),
            frequency: row.frequency,
            next_date: row.next_date.map(seconds),
            is_active: true,
        })
        .collect())
}

/// The currency and amount of an entry, when it is a movement of one thing.
///
/// `None` for a trade, which is two positions and does not fit a row that
/// holds one.
fn transfer(entry: &Entry) -> Option<(&str, f64)> {
    match &entry.tx {
        Tx::Transfer { pos, .. } => Some((&pos.asset, pos.qty)),
        Tx::Trade { .. } => None,
    }
}
