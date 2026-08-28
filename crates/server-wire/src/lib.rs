//! The JSON `server-main` sends and the phone reads.
//!
//! One definition rather than two, which is the whole reason this is its own
//! crate. The obvious home would be `server-main`, but `phone-state` is a
//! `cdylib` linked into iOS and Android and cannot depend on anything that
//! drags in axum and tokio — so the shared half sits here, with nothing in its
//! dependency list but the value types the protocol carries.
//!
//! Every type is both `Serialize` and `Deserialize`: the server writes, the app
//! reads, and a field renamed on one side stops compiling on the other rather
//! than silently arriving as `None`.

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
/// One document rather than an endpoint per screen: the alternative puts
/// "which of my four fetches has landed?" state into the client, which is the
/// state `docs/ARCHITECTURE.md` asks us to remove.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WireSnapshot {
    /// Newest first.
    #[serde(default)]
    pub entries: Vec<WireEntry>,

    #[serde(default)]
    pub connections: Vec<WireConnection>,

    #[serde(default)]
    pub streams: Vec<WireStream>,

    /// When this was assembled.
    ///
    /// The app needs a clock for "in 3 days" and cannot trust its own — a
    /// phone with a wrong date would render a confidently wrong screen.
    #[serde(default)]
    pub now: Timestamp,
}

impl WireSnapshot {
    pub fn new(
        entries: &[Entry],
        connections: &[Connection],
        streams: &[Stream],
        now: Timestamp,
    ) -> Self {
        Self {
            entries: entries.iter().map(WireEntry::from_entry).collect(),
            connections: connections
                .iter()
                .map(WireConnection::from_connection)
                .collect(),
            streams: streams.iter().map(WireStream::from_stream).collect(),
            now,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
    /// index, so inserting a category cannot silently re-file old data.
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

#[derive(Debug, Serialize, Deserialize)]
pub struct WireConnection {
    pub id: String,
    pub name: String,
    pub initials: String,
    /// `"ok"` or `"login_required"`.
    pub status: String,
    /// When this connection last synced. A timestamp rather than "synced 2m
    /// ago": the phrase goes stale the moment it is sent, and only the app's
    /// machine knows what "now" is by the time it renders.
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
            status: match self.status.as_str() {
                "login_required" => ConnectionStatus::LoginRequired,
                _ => ConnectionStatus::Ok {
                    synced_at: self.synced_at,
                },
            },
            accounts: self.accounts.iter().map(WireAccount::to_account).collect(),
        }
    }

    fn from_connection(c: &Connection) -> Self {
        let (status, synced_at) = match &c.status {
            ConnectionStatus::Ok { synced_at } => ("ok", *synced_at),
            ConnectionStatus::LoginRequired => ("login_required", Timestamp::EPOCH),
        };

        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            initials: c.initials.clone(),
            status: status.to_string(),
            synced_at,
            accounts: c.accounts.iter().map(WireAccount::from_account).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WireAccount {
    pub id: String,
    pub name: String,
    /// The source's raw subtype, e.g. `"401k"`. Not worded — the phone words
    /// it, the same as it words everything else it prints.
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

#[derive(Debug, Serialize, Deserialize)]
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

/// The wire name of a category.
///
/// Written out rather than derived from the `Debug` name, so that renaming a
/// Rust variant is a deliberate wire change and not an accidental one. The app
/// has the matching half in `phone-state`.
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

/// A category's wire name back into the enum.
///
/// Unknown names degrade rather than failing: a server that learns a
/// seventeenth category should cost one uncategorised row, not a whole sync.
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
