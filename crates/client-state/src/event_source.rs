//! A source answered, or failed to.

use crate::LoadFailure;

/// What a source came back with. No variant carries a clock reading.
#[derive(Debug, uniffi::Enum)]
pub enum EventSource {
    /// A snapshot arrived. Everything the app knows, in one value.
    ///
    /// JSON rather than a structured record, which would mean redeclaring
    /// every wire type as a UniFFI record and keeping the two in step by hand.
    SnapshotLoaded { json: String },

    /// A request did not come back with what was asked for.
    ///
    /// A reason and not a sentence: the runtime says what it got, and what
    /// that means — a banner, first run, the paywall — is decided here.
    LoadFailed { failure: LoadFailure },

    /// Whether the device has a session to act on. A bool rather than the
    /// credential, which stays the runtime's and never crosses this boundary.
    SessionLoaded { signed_in: bool },

    /// A code went out.
    CodeSent,

    /// The code was the right one.
    CodeAccepted,

    /// It was not.
    ///
    /// No reason: wrong, too late and guessed at too often answer alike, so
    /// nobody can ask whether an address has a code standing against it.
    CodeRejected,

    /// A deletion code went out.
    DeleteCodeSent,

    /// The deletion code was refused — wrong, too late and guessed at too
    /// often answer alike, the way sign-in codes do.
    DeleteCodeRejected,

    /// The account is gone, rows and all. What is left on this device is a
    /// session naming nobody, and the machine puts itself back to first run.
    AccountDeleted,

    /// Plaid's page is up, in a window of its own.
    BankLinkOpened,

    /// Plaid came back with at least one bank attached.
    ///
    /// No name, because nobody has one yet — it arrives on
    /// [`SyncStatusLoaded`](EventSource::SyncStatusLoaded). No count either:
    /// one bank and two lead to the same screen.
    BankLinked,

    /// The bank sign-in ended with nothing — dismissed, failed, timed out.
    BankAbandoned,

    /// The bank sign-in never opened, because the device would not show
    /// Plaid's page — a browser refusing the window it opens in. Nothing was
    /// attempted, which is not the same fact as a sign-in that closed.
    BankBlocked,

    /// The server has not been told what the sign-in came to, because the
    /// browser coming back and Plaid telling the server race. The bank may
    /// yet be attached.
    BankPending,

    /// How far the first sync has got, as `server_wire::WireSyncStatus` in
    /// JSON, for the reason [`SnapshotLoaded`](EventSource::SnapshotLoaded) is.
    SyncStatusLoaded { json: String },

    /// The stream carrying those went away — backgrounded, out of signal, or
    /// refused. A fact and not a verdict: whether to open another one is the
    /// machine's, and it decides that from where the flow is.
    SyncWatchEnded,

    /// Stripe's page is up, in a window of its own.
    ///
    /// Nothing else: the page's address stays with the runtime, the way the
    /// session token does.
    CheckoutOpened,

    /// Stripe's page went away with the money taken.
    ///
    /// Not a subscription the server will honour yet. Stripe tells the server
    /// separately, a moment later, and until it has the server goes on
    /// refusing.
    PaymentTaken,

    /// Stripe's page went away without taking anything — closed by hand, or
    /// backed out of. Nothing was charged and nothing was created.
    CheckoutClosed,
}
