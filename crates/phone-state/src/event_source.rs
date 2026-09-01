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

    /// How far the first sync has got.
    ///
    /// JSON for the same reason [`SnapshotLoaded`](EventSource::SnapshotLoaded)
    /// is: the machine parses it, so all three platforms get the same parse and
    /// there is one place that decides what a missing field means. It is
    /// `server_wire::WireSyncStatus` on the way out of the server.
    ///
    /// One event rather than a progressed/finished pair, because the server
    /// answers one question with one document — whether anything is still
    /// running is a field in it, not a different reply.
    SyncStatusLoaded { json: String },
}
