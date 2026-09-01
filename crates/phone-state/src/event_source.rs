//! A source answered, or failed to.

/// Note that neither variant carries a clock reading. The runtime hands the
/// time to `handle_events` once per batch; a source's job is to say what came
/// back, not when it happened to arrive.
#[derive(Debug, uniffi::Enum)]
pub enum EventSource {
    /// A snapshot arrived. Everything the app knows, in one value.
    ///
    /// The payload is JSON rather than a structured record because the
    /// alternative is redeclaring every wire type as a UniFFI record and
    /// keeping the two in step by hand. The machine parses it, which keeps the
    /// parse in one place for all three platforms.
    SnapshotLoaded { json: String },

    /// A request did not come back.
    LoadFailed { message: String },

    /// Whether the device has a session to act on.
    ///
    /// A bool rather than the address, because that is the whole of what the
    /// machine does with it: a session decides which screen the app opens on
    /// and nothing else. The credential itself is the runtime's — it never
    /// crosses this boundary, so a recorded log carries no way into anybody's
    /// bank. `docs/ARCHITECTURE.md` has the rule this follows from.
    SessionLoaded { signed_in: bool },

    /// A code went out.
    CodeSent,

    /// The code was the right one.
    CodeAccepted,

    /// It was not.
    ///
    /// No reason, and the app does not work one out either. The server knows
    /// whether a code was wrong, too late or guessed at too often — it writes
    /// all three to its own log — and answers alike for every one of them, so
    /// that nobody can ask it whether an address has a code standing against
    /// it. A screen saying "that one expired" would give back exactly what
    /// answering alike was protecting.
    CodeRejected,

    /// Plaid came back with a bank attached, and this is what it is called.
    BankLinked { institution: String },

    /// The bank sign-in ended with nothing — dismissed, failed, timed out.
    /// The common ending, not a rare one.
    BankAbandoned,

    /// How far the first sync has got, and what it has so far.
    ///
    /// The stage and the counts are separate facts: the transactions row reads
    /// `48 so far` while it is still running, so a count is not a claim that
    /// the stage is done.
    SyncProgressed {
        stage: SyncStage,
        accounts: u32,
        transactions: u32,
        streams: u32,
    },

    /// The whole history is in. `/transactions/sync` pages all of it the first
    /// time and almost nothing every time after, which is why this happens
    /// once per bank and never again.
    SyncFinished,
}

/// Which part of the first sync is running.
///
/// In the order they happen, because that order is what the progress bar is a
/// fraction of.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SyncStage {
    #[default]
    Accounts,
    Transactions,
    Streams,
}
