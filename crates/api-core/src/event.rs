//! What happened. See `docs/ARCHITECTURE.md` — events go in, actions come out,
//! and nothing in between touches the outside world.

/// Where an event came from. Grouping by origin is what keeps a match on
/// `Event` readable as the app grows, and makes a recorded log read as a story.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Event {
    FromUi { event: UiEvent },
    FromSource { event: SourceEvent },
}

/// The user did something.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum UiEvent {
    /// A screen came into view and wants whatever it shows.
    ScreenOpened { screen: Screen },
    /// The user picked a transaction out of the list.
    TxSelected { index: u32 },
}

/// A source answered, or failed to.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum SourceEvent {
    /// Transactions arrived from wherever the ledger lives.
    TxsLoaded { txs: Vec<TxSummary> },
    /// A request did not come back.
    LoadFailed { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Screen {
    Transactions,
}

/// One transaction, already formatted for display.
///
/// Deliberately not `api_ledger::Tx`: every formatting decision that crosses
/// as a raw number is a decision two more codebases have to make the same way.
/// The core decides what the row says; the platform decides how it looks.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TxSummary {
    /// "Trade" or "Deposit" — what the row calls itself.
    pub kind: String,
    /// The date, as the row shows it.
    pub date: String,
    /// What left, if anything did.
    pub from: Option<String>,
    /// What arrived.
    pub to: String,
}
