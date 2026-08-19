//! What the server hands the phone, and what the phone reads back.
//!
//! One document rather than three endpoints the client has to join up. The
//! phone gets everything it needs to draw every screen in a single response,
//! which keeps the client free of any "which of my three fetches has landed"
//! state — the exact kind of state `docs/ARCHITECTURE.md` asks us to minimise.
//!
//! These are wire types, so they are `serde` shapes rather than the ledger's.
//! The client turns them back into ledger types on arrival.
//!
//! Its own crate so that the two ends share one definition. A server struct
//! and a hand-written client mirror of it drift the moment either changes, and
//! nothing would fail to compile when they did.

use serde::Deserialize;
use serde::Serialize;

use api_ledger::Account;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Stream;

/// Everything the app knows, as of one moment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// Newest first.
    pub entries: Vec<WireEntry>,

    pub connections: Vec<WireConnection>,

    pub streams: Vec<WireStream>,

    /// When this was assembled, as a unix timestamp.
    ///
    /// The client needs a clock for "in 3 days" and cannot trust its own — a
    /// phone with a wrong date would render a confidently wrong screen. It
    /// also cannot ask for one inside the state machine, which reads no clock
    /// by design. So the server's clock rides along with the data and enters
    /// the machine as an event field.
    pub now: u64,

    /// Whether this came from the live Plaid Sandbox or from the fixtures.
    /// Shown nowhere; logged, and useful when a number looks wrong.
    pub source: SnapshotSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    /// Fetched from Plaid Sandbox just now.
    Live,
    /// The captured Sandbox responses in `fixtures/`.
    #[default]
    Fixture,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireEntry {
    pub id: String,
    pub ts: u64,
    pub amount: f64,
    pub asset: String,
    pub label: String,
    pub account_id: Option<String>,
    pub logo_url: Option<String>,
    /// The category's Rust name, e.g. `"FoodAndDrink"`. A name rather than an
    /// index, so inserting a category cannot silently re-file old data.
    pub category: String,
    pub pending: bool,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireConnection {
    pub id: String,
    pub name: String,
    pub initials: String,
    /// `"ok"` or `"login_required"`.
    pub status: String,
    pub synced: String,
    pub accounts: Vec<WireAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireAccount {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub mask: Option<String>,
    pub balance: f64,
    pub available: Option<f64>,
    pub limit: Option<f64>,
    pub apy: Option<f64>,
    /// `"depository"`, `"investment"`, `"credit"`, `"loan"`, `"other"`.
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireStream {
    pub id: String,
    pub label: String,
    pub logo_url: Option<String>,
    pub category: String,
    pub amount: f64,
    pub frequency: String,
    pub next_date: Option<u64>,
    pub is_active: bool,
}

// ── ledger types -> wire ────────────────────────────────────────────────────

impl Snapshot {
    pub fn new(
        entries: &[Entry],
        connections: &[Connection],
        streams: &[Stream],
        now: u64,
        source: SnapshotSource,
    ) -> Self {
        Self {
            entries: entries.iter().map(WireEntry::from_entry).collect(),
            connections: connections
                .iter()
                .map(WireConnection::from_connection)
                .collect(),
            streams: streams.iter().map(WireStream::from_stream).collect(),
            now,
            source,
        }
    }
}

impl WireEntry {
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

    pub fn to_entry(&self) -> Entry {
        Entry {
            id: self.id.clone(),
            tx: api_ledger::Tx::Transfer {
                ts: self.ts,
                pos: api_ledger::Position {
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
}

impl WireConnection {
    fn from_connection(c: &Connection) -> Self {
        let (status, synced) = match &c.status {
            ConnectionStatus::Ok { synced } => ("ok", synced.clone()),
            ConnectionStatus::LoginRequired => ("login_required", String::new()),
        };

        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            initials: c.initials.clone(),
            status: status.to_string(),
            synced,
            accounts: c.accounts.iter().map(WireAccount::from_account).collect(),
        }
    }

    pub fn to_connection(&self) -> Connection {
        Connection {
            id: self.id.clone(),
            name: self.name.clone(),
            initials: self.initials.clone(),
            status: match self.status.as_str() {
                "login_required" => ConnectionStatus::LoginRequired,
                _ => ConnectionStatus::Ok {
                    synced: self.synced.clone(),
                },
            },
            accounts: self.accounts.iter().map(WireAccount::to_account).collect(),
        }
    }
}

impl WireAccount {
    fn from_account(a: &Account) -> Self {
        Self {
            id: a.id.clone(),
            name: a.name.clone(),
            kind: a.kind.clone(),
            mask: a.mask.clone(),
            balance: a.balance,
            available: a.available,
            limit: a.limit,
            apy: a.apy,
            group: group_name(a.group).to_string(),
        }
    }

    fn to_account(&self) -> Account {
        Account {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            mask: self.mask.clone(),
            balance: self.balance,
            available: self.available,
            limit: self.limit,
            apy: self.apy,
            group: group_from_name(&self.group),
        }
    }
}

impl WireStream {
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
}

/// The wire name of a category.
///
/// Written out rather than derived, so that renaming a Rust variant is a
/// deliberate wire change and not an accidental one.
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

fn category_from_name(name: &str) -> Category {
    Category::ALL
        .iter()
        .copied()
        .find(|c| category_name(*c) == name)
        .unwrap_or(Category::Unknown)
}

fn group_name(g: api_ledger::AccountGroup) -> &'static str {
    use api_ledger::AccountGroup as G;
    match g {
        G::Depository => "depository",
        G::Investment => "investment",
        G::Credit => "credit",
        G::Loan => "loan",
        G::Other => "other",
    }
}

fn group_from_name(name: &str) -> api_ledger::AccountGroup {
    api_ledger::AccountGroup::from_plaid(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_survives_a_round_trip() {
        for c in Category::ALL {
            assert_eq!(category_from_name(category_name(c)), c, "{c:?}");
        }
    }

    #[test]
    fn every_group_survives_a_round_trip() {
        use api_ledger::AccountGroup as G;

        for g in [G::Depository, G::Investment, G::Credit, G::Loan, G::Other] {
            assert_eq!(group_from_name(group_name(g)), g, "{g:?}");
        }
    }
}
