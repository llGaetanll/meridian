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

    /// Ask how far the first sync has got. One question, not a subscription:
    /// asking again is the machine's to decide on the tick, so every poll is
    /// an action in the log.
    PollSync,
}
