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

    /// Throw the session away, so that the next launch opens on first run.
    ///
    /// The counterpart to [`SaveSession`](ActionSource::SaveSession), and the
    /// only way out of a session the server no longer honours — a token good
    /// for ninety days that the server has stopped accepting answers 401 to
    /// everything, and there is nothing the app can do about it but forget it.
    ForgetSession,

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

    /// Ask how far the first sync has got.
    ///
    /// One question, not a subscription: the runtime makes one request and
    /// sends one event back. Asking *again* is the machine's to decide, and it
    /// does so on the tick — so every poll is an action in the log and a
    /// replay makes the same number of them in the same places. A loop in the
    /// runtime would be a decision no log records.
    ///
    /// The work is the server's — it holds the `access_token` and runs the
    /// three calls — so this only ever reads. Backgrounding the app while it
    /// runs is safe for the same reason.
    ///
    /// It is a poll because the server can only answer questions it is asked.
    /// Issue #30 is the push channel this wants; when it lands, this action
    /// and the tick rule that repeats it are the whole of what goes away.
    PollSync,
}
