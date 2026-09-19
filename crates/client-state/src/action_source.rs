//! Something for a source to do — a source being anything outside the machine
//! that can be asked a question.

#[derive(Debug, uniffi::Enum)]
pub enum ActionSource {
    /// Go and get everything. No freshness hint: when to ask a bank again is
    /// the server's decision, since a hint the app can repeat at will spends
    /// an account-wide Plaid budget.
    LoadSnapshot,

    /// Read who the device says is signed in.
    LoadSession,

    /// Commit the session the runtime is holding, so that the next launch
    /// opens on the Ledger. No payload: the credential is the one `VerifyCode`
    /// came back with, and the runtime has held it since.
    SaveSession,

    /// Throw the session away, so that the next launch opens on first run.
    ForgetSession,

    /// Send a six-digit code to this address.
    SendCode { email: String },

    /// Check a six-digit code against the address it was sent to.
    VerifyCode { email: String, code: String },

    /// Ask the server for a `link_token` and open the `hosted_link_url` that
    /// comes back. The app never holds a token of either kind: the secret that
    /// signs both cannot ship inside an app (`docs/PLAID.md`).
    StartBankLink,

    /// The same, in update mode against one bank the user already holds: ask
    /// the server for a repair session and open its page. The sign-in renews
    /// the bank's consent instead of attaching a new one.
    StartBankRepair { item_id: String },

    /// Ask the server for a Stripe Checkout page and open it in a window of
    /// its own. The money is taken there: no card detail ever reaches this
    /// app, and nothing about the page is a thing to put in an event log.
    StartCheckout,

    /// Open a stream the server writes the sync's progress to, and report
    /// what arrives on it.
    ///
    /// The runtime holds the connection and chooses how to read it; when to
    /// have one at all, and when to try again after losing one, are decisions
    /// and stay here — so a replay reproduces every attempt.
    WatchSync,

    /// Let that stream go. Its own action rather than something the runtime
    /// infers from a screen it cannot see: a connection nobody closes is a
    /// connection the server holds a slot for.
    StopWatchingSync,
}
