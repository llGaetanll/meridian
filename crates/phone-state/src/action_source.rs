//! Something for a source to do.
//!
//! A source here is anything outside the machine that can be asked a question:
//! the server, Plaid by way of the server, and the store on the device that
//! remembers who is signed in.

#[derive(Debug, uniffi::Enum)]
pub enum ActionSource {
    /// Go and get everything. `force` skips whatever cache sits in the way,
    /// which is what pull-to-refresh means.
    LoadSnapshot {
        force: bool,
    },

    /// Read who the device says is signed in. The first thing the app asks
    /// for, because the answer decides whether it opens on the Ledger or on
    /// the first screen of first run.
    LoadSession,

    /// Commit the session the runtime is holding, so that the next launch
    /// opens on the Ledger.
    ///
    /// No payload: the credential to write is the one `VerifyCode` came back
    /// with, and the runtime has held it since. This is the machine saying
    /// *now* — first run is through, and what was good enough to live in
    /// memory is worth keeping.
    SaveSession,

    /// Send a six-digit code to this address.
    SendCode {
        email: String,
    },

    VerifyCode {
        email: String,
        code: String,
    },

    /// Ask the server for a `link_token` and open the `hosted_link_url` that
    /// comes back, which is Plaid's own page over the web.
    ///
    /// The app never holds a token of either kind: the server creates the link
    /// token and the server exchanges the public token, because the secret
    /// that signs both cannot ship inside an app (`docs/PLAID.md`).
    StartBankLink,

    /// Follow the first sync, and report what lands.
    ///
    /// The work is the server's — it holds the `access_token` and runs
    /// `/transactions/sync` — so this asks to be told about it rather than to
    /// do it. Backgrounding the app while it runs is safe for the same reason.
    WatchSync,
}
