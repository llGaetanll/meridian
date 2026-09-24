//! The JSON `server-main` sends and the clients read.
//!
//! Its own crate because `client-state` is a `cdylib` linked into iOS and
//! Android and cannot depend on anything that drags in axum and tokio. Every
//! type is both `Serialize` and `Deserialize`, so a renamed field stops
//! compiling on the other side rather than arriving as `None`.

use api_common::Timestamp;
use api_ledger::Account;
use api_ledger::AccountGroup;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Position;
use api_ledger::Stream;
use api_ledger::Tx;
use serde::Deserialize;
use serde::Serialize;

/// Everything the app knows, as of one moment.
///
/// Carries the timestamps *of* things and no clock: what time it is now is the
/// device's to know.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WireSnapshot {
    /// Who the device is signed in as.
    ///
    /// Here rather than on a call of its own because the app asks for a
    /// snapshot already and this changes about as often as the ledger does —
    /// a second request to print two lines in a corner is a second thing that
    /// can be in flight, fail, or arrive late.
    ///
    /// Optional on the wire so an older server and a newer app do not have to
    /// be deployed in one move; `None` is a chip with nothing on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<WireUser>,

    /// Newest first.
    #[serde(default)]
    pub entries: Vec<WireEntry>,

    #[serde(default)]
    pub connections: Vec<WireConnection>,

    #[serde(default)]
    pub streams: Vec<WireStream>,
}

impl WireSnapshot {
    pub fn new(
        user: Option<WireUser>,
        entries: &[Entry],
        connections: &[Connection],
        streams: &[Stream],
    ) -> Self {
        Self {
            user,
            entries: entries.iter().map(WireEntry::from_entry).collect(),
            connections: connections
                .iter()
                .map(WireConnection::from_connection)
                .collect(),
            streams: streams.iter().map(WireStream::from_stream).collect(),
        }
    }
}

/// Who the device is signed in as, for the one corner of the app that says so.
///
/// Nothing here is a credential: the address is printed, never sent back to
/// prove anything, and the session token remains the only thing that does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireUser {
    pub email: String,

    /// Absent until they have been asked. Every account made before first run
    /// asked has none, and there is nowhere to get one from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireEntry {
    pub id: String,
    pub ts: Timestamp,
    pub amount: f64,
    pub asset: String,
    pub label: String,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub logo_url: Option<String>,
    /// The category's Rust name, e.g. `"FoodAndDrink"`. A name rather than an
    /// index, so inserting a category cannot re-file old data.
    pub category: String,
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub supersedes: Option<String>,
}

impl WireEntry {
    pub fn to_entry(&self) -> Entry {
        Entry {
            id: self.id.clone(),
            tx: Tx::Transfer {
                ts: self.ts,
                pos: Position {
                    asset: self.asset.clone(),
                    qty: self.amount,
                },
            },
            label: self.label.clone(),
            account_id: self.account_id.clone(),
            logo_url: self.logo_url.clone(),
            category: category_from_name(&self.category),
            pending: self.pending,
            supersedes: self.supersedes.clone(),
        }
    }

    fn from_entry(e: &Entry) -> Self {
        let asset = match &e.tx {
            api_ledger::Tx::Transfer { pos, .. } => pos.asset.clone(),
            api_ledger::Tx::Trade { sell, .. } => sell.asset.clone(),
        };

        Self {
            id: e.id.clone(),
            ts: e.ts(),
            amount: e.amount(),
            asset,
            label: e.label.clone(),
            account_id: e.account_id.clone(),
            logo_url: e.logo_url.clone(),
            category: category_name(e.category).to_string(),
            pending: e.pending,
            supersedes: e.supersedes.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireConnection {
    pub id: String,
    pub name: String,
    pub initials: String,
    /// The status's name, e.g. `"login_required"`.
    pub status: String,
    /// When this connection last synced. A timestamp rather than "synced 2m
    /// ago", which goes stale the moment it is sent.
    #[serde(default)]
    pub synced_at: Timestamp,
    #[serde(default)]
    pub accounts: Vec<WireAccount>,
}

impl WireConnection {
    pub fn to_connection(&self) -> Connection {
        Connection {
            id: self.id.clone(),
            name: self.name.clone(),
            initials: self.initials.clone(),
            status: connection_status_from_name(&self.status, self.synced_at),
            accounts: self.accounts.iter().map(WireAccount::to_account).collect(),
        }
    }

    fn from_connection(c: &Connection) -> Self {
        let synced_at = match &c.status {
            ConnectionStatus::Ok { synced_at } => *synced_at,
            ConnectionStatus::LoginRequired => Timestamp::EPOCH,
        };

        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            initials: c.initials.clone(),
            status: connection_status_name(&c.status).to_string(),
            synced_at,
            accounts: c.accounts.iter().map(WireAccount::from_account).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireAccount {
    pub id: String,
    pub name: String,
    /// The source's raw subtype, e.g. `"401k"`. Wording it is the phone's job.
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub mask: Option<String>,
    pub balance: f64,
    #[serde(default)]
    pub available: Option<f64>,
    #[serde(default)]
    pub limit: Option<f64>,
    #[serde(default)]
    pub apy: Option<f64>,
    /// `"depository"`, `"investment"`, `"credit"`, `"loan"`, `"other"`.
    pub group: String,
}

impl WireAccount {
    fn to_account(&self) -> Account {
        Account {
            id: self.id.clone(),
            name: self.name.clone(),
            subtype: self.subtype.clone(),
            mask: self.mask.clone(),
            balance: self.balance,
            available: self.available,
            limit: self.limit,
            apy: self.apy,
            group: AccountGroup::from_plaid(&self.group),
        }
    }

    fn from_account(a: &Account) -> Self {
        Self {
            id: a.id.clone(),
            name: a.name.clone(),
            subtype: a.subtype.clone(),
            mask: a.mask.clone(),
            balance: a.balance,
            available: a.available,
            limit: a.limit,
            apy: a.apy,
            group: a.group.as_plaid().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireStream {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub logo_url: Option<String>,
    pub category: String,
    pub amount: f64,
    pub frequency: String,
    #[serde(default)]
    pub next_date: Option<Timestamp>,
    #[serde(default)]
    pub is_active: bool,
}

impl WireStream {
    pub fn to_stream(&self) -> Stream {
        Stream {
            id: self.id.clone(),
            label: self.label.clone(),
            logo_url: self.logo_url.clone(),
            category: category_from_name(&self.category),
            amount: self.amount,
            frequency: self.frequency.clone(),
            next_date: self.next_date,
            is_active: self.is_active,
        }
    }

    fn from_stream(s: &Stream) -> Self {
        Self {
            id: s.id.clone(),
            label: s.label.clone(),
            logo_url: s.logo_url.clone(),
            category: category_name(s.category).to_string(),
            amount: s.amount,
            frequency: s.frequency.clone(),
            next_date: s.next_date,
            is_active: s.is_active,
        }
    }
}

/// How far the first sync has got.
///
/// Its own document rather than a field on [`WireSnapshot`], which cannot
/// answer until the fetch it triggered is over; this is read while one runs.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WireSyncStatus {
    /// The stage's name, e.g. `"transactions"`. A name rather than an index,
    /// so inserting a stage cannot rename the others.
    pub stage: String,

    /// What the bank is called, once anyone knows. Absent until the balances
    /// call inside the first sync answers, which is the first moment it exists.
    #[serde(default)]
    pub institution: Option<String>,

    #[serde(default)]
    pub accounts: u32,

    #[serde(default)]
    pub transactions: u32,

    #[serde(default)]
    pub streams: u32,

    /// Whether there is anything still running. Its own field because the last
    /// stage looks identical whether it is running or done.
    #[serde(default)]
    pub finished: bool,
}

impl WireSyncStatus {
    /// What a user with no sync in flight is told.
    pub fn idle() -> Self {
        Self {
            stage: stage_name(SyncStage::Accounts).to_string(),
            finished: true,
            ..Self::default()
        }
    }

    pub fn new(
        stage: SyncStage,
        institution: Option<String>,
        accounts: u32,
        transactions: u32,
        streams: u32,
    ) -> Self {
        Self {
            stage: stage_name(stage).to_string(),
            institution,
            accounts,
            transactions,
            streams,
            finished: false,
        }
    }

    /// A sync that is over, and what it brought. The counts are what *this*
    /// sync added, not the totals the store holds.
    pub fn done(
        institution: Option<String>,
        accounts: u32,
        transactions: u32,
        streams: u32,
    ) -> Self {
        Self {
            stage: stage_name(SyncStage::Streams).to_string(),
            institution,
            accounts,
            transactions,
            streams,
            finished: true,
        }
    }

    pub fn stage(&self) -> SyncStage {
        stage_from_name(&self.stage)
    }
}

/// Which part of the first sync is running, in the order they happen —
/// the server's calls and the screen's rows both follow it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SyncStage {
    #[default]
    Accounts,
    Transactions,
    Streams,
}

impl SyncStage {
    pub const ALL: [SyncStage; 3] = [
        SyncStage::Accounts,
        SyncStage::Transactions,
        SyncStage::Streams,
    ];
}

/// One series of bars, as `GET /bars` answers it.
///
/// The interval is the one actually served, which is not always the one asked
/// for — the sources substitute, and the client draws what it was given.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireBars {
    pub symbol: String,

    /// `1m` through `1wk`, the words `src_postgres::BarInterval` spells.
    pub interval: String,

    /// In time order. What the gaps mean — market shut, or never fetched — is
    /// not on the wire; a client draws the points it is given.
    #[serde(default)]
    pub bars: Vec<WireBar>,
}

/// One bar. The close is always there; the rest of the candle is whatever the
/// source reported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireBar {
    /// The start of the interval the bar stands for, in seconds.
    pub ts: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,

    pub close: f64,

    pub volume: u64,
}

/// Why a refusal was a 402, so a client can draw the right paywall.
///
/// The only refusal that carries a body. A 401 and a 429 are wholly said by
/// their status code; this one is not, because "you have never subscribed" and
/// "your card stopped working" are the same code and different screens.
///
/// **No provider vocabulary crosses this.** Stripe says `past_due` and Apple
/// says something else again, and a client that matched on either would have
/// learned one provider's dictionary — so a second provider would be a second
/// set of arms in every app. The server maps; the wire carries our word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireUnpaid {
    /// `"never"`, `"declined"`, `"ended"`, `"unfinished"` or `"unconfirmed"`.
    pub reason: String,

    /// The end of the period that was paid for, in seconds. Always in the
    /// past: entitlement is that date against the clock, so a period still
    /// running is a session that was never refused. Absent where there is no
    /// date, which is also every [`UnpaidReason::Never`].
    pub ran_out: Option<u64>,
}

impl WireUnpaid {
    /// Why, as a value. An unknown word reads as
    /// [`Never`](UnpaidReason::Never) — see [`unpaid_reason_from_name`].
    pub fn reason(&self) -> UnpaidReason {
        unpaid_reason_from_name(&self.reason)
    }
}

/// Why somebody signed in is not being served.
///
/// Five words, and they are the five different things a paywall has to say.
/// There is no "paid" arm: this is carried by a refusal, and a session being
/// served never produces one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnpaidReason {
    /// Nobody has ever subscribed on this account. Says nothing about a past,
    /// which is what makes it the safe reading of a word we do not know.
    Never,

    /// A payment did not go through.
    Declined,

    /// A subscription that ended, and the period it paid for has passed.
    Ended,

    /// A subscription that was started and has not been paid for. Nothing
    /// was charged, and the card form picks it back up.
    Unfinished,

    /// We cannot tell. A payment still in flight, or a provider that believes
    /// this person is paying while the date we hold says otherwise.
    ///
    /// Its own word rather than folded into one of the three, because each of
    /// those states a cause — never subscribed, a card refused, a subscription
    /// ended — and this is exactly the case where none of them is known to be
    /// true. Somebody mid-payment told their subscription has ended is being
    /// lied to.
    Unconfirmed,
}

impl UnpaidReason {
    pub const ALL: [UnpaidReason; 5] = [
        UnpaidReason::Never,
        UnpaidReason::Declined,
        UnpaidReason::Ended,
        UnpaidReason::Unfinished,
        UnpaidReason::Unconfirmed,
    ];
}

/// The wire name of a reason. Written out rather than derived from `Debug`, so
/// renaming a Rust variant is a deliberate wire change.
pub fn unpaid_reason_name(reason: UnpaidReason) -> &'static str {
    match reason {
        UnpaidReason::Never => "never",
        UnpaidReason::Declined => "declined",
        UnpaidReason::Ended => "ended",
        UnpaidReason::Unfinished => "unfinished",
        UnpaidReason::Unconfirmed => "unconfirmed",
    }
}

/// A reason's wire name back into the enum.
///
/// An unknown word reads as `never`, whose screen claims nothing about what
/// came before — so a server that learns a fourth reason before a client does
/// makes that client say less rather than something untrue.
pub fn unpaid_reason_from_name(name: &str) -> UnpaidReason {
    UnpaidReason::ALL
        .iter()
        .copied()
        .find(|reason| unpaid_reason_name(*reason) == name)
        .unwrap_or(UnpaidReason::Never)
}

/// The wire name of a stage. Written out rather than derived from `Debug`, so
/// renaming the variant is a deliberate wire change and not an accident.
pub fn stage_name(stage: SyncStage) -> &'static str {
    match stage {
        SyncStage::Accounts => "accounts",
        SyncStage::Transactions => "transactions",
        SyncStage::Streams => "streams",
    }
}

/// A stage's wire name back into the enum. An unknown name reads as the first
/// stage, which costs a progress bar that starts low rather than a failure.
pub fn stage_from_name(name: &str) -> SyncStage {
    SyncStage::ALL
        .iter()
        .copied()
        .find(|stage| stage_name(*stage) == name)
        .unwrap_or(SyncStage::Accounts)
}

/// The wire name of a connection that is syncing.
const STATUS_OK: &str = "ok";

/// The wire name of a connection whose bank wants a fresh sign-in.
const STATUS_LOGIN_REQUIRED: &str = "login_required";

/// The wire name of a connection's status. Written out rather than derived
/// from `Debug`, so renaming a Rust variant is a deliberate wire change.
fn connection_status_name(status: &ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Ok { .. } => STATUS_OK,
        ConnectionStatus::LoginRequired => STATUS_LOGIN_REQUIRED,
    }
}

/// A status's wire name back into the enum, with the timestamp that rides
/// beside it on the wire. An unknown name reads as working, which costs a
/// stale "synced 2m ago" rather than a bank the user is told to reconnect.
fn connection_status_from_name(name: &str, synced_at: Timestamp) -> ConnectionStatus {
    match name {
        STATUS_LOGIN_REQUIRED => ConnectionStatus::LoginRequired,
        _ => ConnectionStatus::Ok { synced_at },
    }
}

/// The wire name of a category. Written out rather than derived from `Debug`,
/// so renaming a Rust variant is a deliberate wire change.
fn category_name(c: Category) -> &'static str {
    match c {
        Category::Income => "Income",
        Category::TransferIn => "TransferIn",
        Category::TransferOut => "TransferOut",
        Category::LoanPayments => "LoanPayments",
        Category::BankFees => "BankFees",
        Category::Entertainment => "Entertainment",
        Category::FoodAndDrink => "FoodAndDrink",
        Category::GeneralMerchandise => "GeneralMerchandise",
        Category::HomeImprovement => "HomeImprovement",
        Category::Medical => "Medical",
        Category::PersonalCare => "PersonalCare",
        Category::GeneralServices => "GeneralServices",
        Category::GovernmentAndNonProfit => "GovernmentAndNonProfit",
        Category::Transportation => "Transportation",
        Category::Travel => "Travel",
        Category::RentAndUtilities => "RentAndUtilities",
        Category::Unknown => "Unknown",
    }
}

/// A category's wire name back into the enum. An unknown name costs one
/// uncategorised row rather than a whole sync.
fn category_from_name(name: &str) -> Category {
    Category::ALL
        .iter()
        .copied()
        .find(|c| category_name(*c) == name)
        .unwrap_or(Category::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both sides map these back to the enum, and the phone draws its three
    /// rows in this order. A name changed on one side and not the other stops
    /// the screen tracking the sync it is describing.
    #[test]
    fn stage_names_are_stable() {
        let names: Vec<&str> = SyncStage::ALL.iter().copied().map(stage_name).collect();

        assert_eq!(names, ["accounts", "transactions", "streams"]);

        for stage in SyncStage::ALL {
            assert_eq!(stage_from_name(stage_name(stage)), stage);
        }
    }

    /// The phone maps these names back to the enum, and only
    /// `"login_required"` puts the Reconnect button on the bank. A name
    /// changed on one side and not the other leaves a bank that is not
    /// syncing looking fine, so the two are pinned here as text.
    #[test]
    fn connection_status_names_are_stable() {
        assert_eq!(
            connection_status_name(&ConnectionStatus::Ok {
                synced_at: Timestamp::EPOCH
            }),
            "ok",
        );
        assert_eq!(
            connection_status_name(&ConnectionStatus::LoginRequired),
            "login_required",
        );

        assert_eq!(
            connection_status_from_name("login_required", Timestamp::EPOCH),
            ConnectionStatus::LoginRequired,
        );
    }

    /// Every client switches on these words to decide which paywall it
    /// draws, so they are a wire contract and are pinned here as text. A word
    /// changed on one side and not the other tells somebody whose card was
    /// declined that they have never subscribed.
    #[test]
    fn the_reasons_a_paywall_draws_are_named_on_the_wire() {
        assert_eq!(unpaid_reason_name(UnpaidReason::Never), "never");
        assert_eq!(unpaid_reason_name(UnpaidReason::Declined), "declined");
        assert_eq!(unpaid_reason_name(UnpaidReason::Ended), "ended");
        assert_eq!(unpaid_reason_name(UnpaidReason::Unfinished), "unfinished");
        assert_eq!(unpaid_reason_name(UnpaidReason::Unconfirmed), "unconfirmed");

        assert_eq!(unpaid_reason_from_name("declined"), UnpaidReason::Declined);

        // A reason this build has never heard of says the least rather than
        // something untrue.
        assert_eq!(unpaid_reason_from_name("refunded"), UnpaidReason::Never);
    }

    /// The app maps these names back to the enum. A name changed on one side
    /// and not the other silently re-files every row under "Uncategorised",
    /// so the set is pinned here as text.
    #[test]
    fn category_names_are_stable() {
        let names: Vec<&str> = Category::ALL.iter().copied().map(category_name).collect();

        assert_eq!(
            names,
            [
                "Transportation",
                "HomeImprovement",
                "FoodAndDrink",
                "GovernmentAndNonProfit",
                "Travel",
                "TransferIn",
                "BankFees",
                "LoanPayments",
                "GeneralServices",
                "TransferOut",
                "Income",
                "Medical",
                "GeneralMerchandise",
                "RentAndUtilities",
                "PersonalCare",
                "Entertainment",
            ],
        );
    }
}
