//! Plaid's shapes, projected onto the ledger's.
//!
//! This is the one place the two conventions meet, and `docs/PLAID.md` names
//! both ways it can be quietly wrong. Neither looks like a bug from outside —
//! the numbers stay plausible — so both are pinned by tests against the real
//! captured Sandbox responses in `fixtures/`.
//!
//! It lives in the server rather than in `src-plaid` because `src-plaid` holds
//! Plaid's types as Plaid reports them, deliberately: a source crate that
//! quietly renamed and re-signed everything would make the raw response
//! impossible to check a log against.

use api_ledger::Account;
use api_ledger::AccountGroup;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Stream;
use api_ledger::Tx;
use api_ledger::initials;
use src_plaid::BalancesResponse;
use src_plaid::PlaidAccount;
use src_plaid::PlaidStream;
use src_plaid::PlaidTransaction;
use src_plaid::RecurringResponse;

/// The currency an entry is denominated in, when Plaid does not say.
const DEFAULT_CURRENCY: &str = "USD";

/// One Plaid transaction as a ledger entry.
///
/// **The sign flips here, once.** Plaid reports a card purchase as a *positive*
/// `amount` — positive means money left the account. The ledger's convention is
/// the opposite, so a purchase is negative. Doing this in one place is the
/// whole point: a second negation somewhere downstream would cancel this one
/// and turn every expense into income.
pub fn entry(tx: &PlaidTransaction) -> Entry {
    let currency = tx
        .iso_currency_code
        .clone()
        .unwrap_or_else(|| DEFAULT_CURRENCY.to_string());

    let position = Position {
        asset: currency,
        qty: -tx.amount,
    };

    Entry {
        id: tx.transaction_id.clone(),
        // A bank feed reports money moving in or out of an account, not an
        // exchange of two assets, so every entry from Plaid is a Transfer. A
        // Trade needs both sides, which only a brokerage feed carries.
        tx: Tx::Transfer {
            ts: tx.date,
            pos: position,
        },
        label: tx.label().to_string(),
        account_id: Some(tx.account_id.clone()),
        logo_url: tx.logo_url.clone(),
        category: tx
            .category
            .as_ref()
            .map(|c| Category::from_plaid(&c.primary))
            .unwrap_or(Category::Unknown),
        pending: tx.pending,
        supersedes: tx.pending_transaction_id.clone(),
    }
}

/// The whole connection: the institution, its accounts, and its health.
pub fn connection(balances: &BalancesResponse, synced_at: api_common::Timestamp) -> Connection {
    let name = balances
        .item
        .institution_name
        .clone()
        .or_else(|| balances.item.institution_id.clone())
        .unwrap_or_else(|| "Bank".to_string());

    let status = connection_status(
        balances
            .item
            .error
            .as_ref()
            .and_then(|e| e.error_code.as_deref()),
        synced_at,
    );

    Connection {
        id: balances.item.item_id.clone(),
        initials: initials(&name),
        name,
        status,
        accounts: balances.accounts.iter().map(account).collect(),
    }
}

/// Whether a connection is working, from the source's error code.
///
/// Any error at all means it is not syncing, but only `ITEM_LOGIN_REQUIRED` is
/// fixable by the user signing in again. Treating every error as
/// login-required would send them round a loop that cannot help; treating none
/// as it would hide the one action that works.
///
/// Shared by the two ways a connection is built — from a fresh balances
/// response, and from a row read back at startup — so the rule is stated once
/// and cannot come out differently depending on where the data came from.
pub fn connection_status(
    error_code: Option<&str>,
    synced_at: api_common::Timestamp,
) -> ConnectionStatus {
    match error_code {
        Some("ITEM_LOGIN_REQUIRED") => ConnectionStatus::LoginRequired,
        _ => ConnectionStatus::Ok { synced_at },
    }
}

/// One Plaid account.
///
/// The balance keeps Plaid's sign — positive — for both what is held and what
/// is owed, because it is the account's `type` that says which, not the sign.
/// See [`Account::is_debt`].
///
/// The subtype is carried across raw. Wording `"401k"` as `"401(k)"` is a
/// decision about what a screen says, and `phone-state` makes those.
pub fn account(a: &PlaidAccount) -> Account {
    let group = AccountGroup::from_plaid(&a.kind);

    Account {
        id: a.account_id.clone(),
        name: a.name.clone(),
        subtype: a.subtype.clone(),
        mask: a.mask.clone(),
        balance: a.balances.current.unwrap_or(0.0),
        // Only worth showing where it differs from the balance; on most
        // accounts Plaid repeats the same figure and a second identical
        // number on the row says nothing.
        available: a
            .balances
            .available
            .filter(|available| (available - a.balances.current.unwrap_or(0.0)).abs() > 0.005),
        limit: a.balances.limit,
        // Plaid sends 0 rather than null for accounts that pay no interest.
        apy: a.apy.filter(|apy| *apy > 0.0),
        group,
    }
}

/// The recurring streams, both directions, in one list.
///
/// Inactive streams are dropped: Plaid keeps returning a cancelled
/// subscription, and counting it would overstate the monthly commitment the
/// screen leads with.
pub fn streams(recurring: &RecurringResponse) -> Vec<Stream> {
    recurring
        .outflow_streams
        .iter()
        .chain(recurring.inflow_streams.iter())
        .filter(|s| s.is_active.unwrap_or(true))
        .map(stream)
        .collect()
}

fn stream(s: &PlaidStream) -> Stream {
    // Plaid's sign again, and the same flip: on an outflow stream the average
    // is positive, and money leaving is negative in the ledger.
    let amount = -s.average_amount.amount.unwrap_or(0.0);

    // `merchant_name` comes back as "" rather than null when Plaid matched
    // nothing, so an emptiness check is what is needed, not `unwrap_or`.
    let label = s
        .merchant_name
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or(&s.description)
        .to_string();

    Stream {
        id: s.stream_id.clone(),
        label,
        logo_url: None,
        category: s
            .category
            .as_ref()
            .map(|c| Category::from_plaid(&c.primary))
            .unwrap_or(Category::Unknown),
        amount,
        frequency: Stream::frequency_label(&s.frequency),
        next_date: s.predicted_next_date,
        is_active: s.is_active.unwrap_or(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use src_plaid::SyncResponse;

    /// The real captured Sandbox responses, so what these prove is that we
    /// handle *Plaid's* shapes and not our idea of them.
    fn sync() -> SyncResponse {
        serde_json::from_str(include_str!("../fixtures/sync.json")).expect("fixture parses")
    }

    fn balances() -> BalancesResponse {
        serde_json::from_str(include_str!("../fixtures/balances.json")).expect("fixture parses")
    }

    fn recurring() -> RecurringResponse {
        serde_json::from_str(include_str!("../fixtures/recurring.json")).expect("fixture parses")
    }

    /// Trap one from `docs/PLAID.md`. A card purchase is positive to Plaid and
    /// negative to us, and an app that got this backwards would report a
    /// month's spending as a month's income.
    #[test]
    fn a_purchase_becomes_money_leaving() {
        let uber = sync()
            .added
            .into_iter()
            .find(|t| t.label() == "Uber")
            .expect("the fixture has an Uber ride");

        // Plaid's own number is positive.
        assert!(uber.amount > 0.0, "the fixture should hold Plaid's sign");

        assert_eq!(entry(&uber).amount(), -uber.amount);
        assert!(!entry(&uber).is_inflow());
    }

    /// The inverse, on the row the sketch calls out: a −500.00 United Airlines
    /// refund is money arriving.
    #[test]
    fn a_refund_becomes_money_arriving() {
        let refund = sync()
            .added
            .into_iter()
            .find(|t| t.amount < 0.0)
            .expect("the fixture has a negative amount");

        assert!(entry(&refund).is_inflow());
        assert_eq!(entry(&refund).amount(), -refund.amount);
    }

    #[test]
    fn every_fixture_transaction_projects() {
        let sync = sync();
        let entries: Vec<Entry> = sync.added.iter().map(entry).collect();

        assert_eq!(entries.len(), 48, "the captured Sandbox feed has 48");

        // Nothing should land in Unknown: the fixture only carries categories
        // Plaid documents, so an Unknown here means a missing arm.
        assert!(
            entries.iter().all(|e| e.category != Category::Unknown),
            "every fixture category should map",
        );

        assert!(entries.iter().all(|e| !e.label.is_empty()));
    }

    /// Trap two's carrier: the pointer at the pending transaction has to
    /// survive projection, or `Ledger::apply` has nothing to work with.
    #[test]
    fn the_pending_pointer_is_carried_across() {
        let mut tx = sync().added.remove(0);
        tx.pending_transaction_id = Some("pending-1".to_string());

        assert_eq!(entry(&tx).supersedes.as_deref(), Some("pending-1"));
    }

    #[test]
    fn accounts_project_with_debts_marked() {
        let conn = connection(&balances(), api_common::Timestamp::from_secs(1_787_097_600));

        assert_eq!(conn.name, "First Platypus Bank");
        assert_eq!(conn.initials, "FP");
        assert_eq!(conn.accounts.len(), 14);

        let debts: Vec<&Account> = conn.accounts.iter().filter(|a| a.is_debt()).collect();

        // 2 credit cards + 4 loans in the Sandbox feed.
        assert_eq!(debts.len(), 6);
        assert!(
            debts.iter().all(|a| a.balance > 0.0),
            "Plaid's sign is kept"
        );

        // The figures the Balances screen leads with.
        assert!(
            (conn.held() - 86_541.74).abs() < 0.01,
            "held: {}",
            conn.held()
        );
        assert!(
            (conn.owed() - -163_705.89).abs() < 0.01,
            "owed: {}",
            conn.owed()
        );
    }

    #[test]
    fn a_mortgage_is_never_counted_as_savings() {
        let conn = connection(&balances(), api_common::Timestamp::from_secs(1_787_097_600));

        let mortgage = conn
            .accounts
            .iter()
            .find(|a| a.name == "Plaid Mortgage")
            .expect("the fixture has a mortgage");

        assert!(mortgage.is_debt());
        assert_eq!(mortgage.subtype.as_deref(), Some("mortgage"));
    }

    /// An APY of zero is nothing to say, so the row should not say it.
    #[test]
    fn only_real_interest_is_kept() {
        let conn = connection(&balances(), api_common::Timestamp::from_secs(1_787_097_600));

        let checking = conn
            .accounts
            .iter()
            .find(|a| a.name == "Plaid Checking")
            .unwrap();
        assert_eq!(checking.apy, None);
    }

    #[test]
    fn streams_project_with_the_sign_flipped() {
        let streams = streams(&recurring());

        assert_eq!(streams.len(), 8, "7 out and 1 in");

        let payment = streams
            .iter()
            .find(|s| s.label == "AUTOMATIC PAYMENT - THANK")
            .expect("the fixture has this stream");

        // Plaid says +2078.50 going out; the ledger says −2078.50.
        assert_eq!(payment.amount, -2078.50);
        assert!(payment.is_outflow());
        assert_eq!(payment.frequency, "Monthly");

        let interest = streams.iter().find(|s| s.label == "INTRST PYMNT").unwrap();
        assert!(!interest.is_outflow());
        assert_eq!(interest.amount, 4.22);
    }

    /// The monthly commitment the Recurring screen leads with.
    #[test]
    fn the_monthly_outflow_matches_the_sketch() {
        let streams = streams(&recurring());

        let out: f64 = streams
            .iter()
            .filter(|s| s.is_outflow())
            .map(|s| s.monthly_amount())
            .sum();

        assert!((out - -3_771.40).abs() < 0.01, "got {out}");
    }
}
