//! Something for the store to do.
//!
//! Only writes. Reads are not actions: the runtime reads a user's rows on the
//! request path and serves them, because the alternative — holding every
//! user's ledger in memory — is what stopped this server being able to serve
//! more than one person.
//!
//! Nothing here carries an access token. The machine has never seen one — it
//! lives in the runtime, beside the credentials it belongs with — and an action
//! that carried it would put a bank credential into the event log.

use crate::CallId;

use api_ledger::Account;
use api_ledger::Entry;
use api_ledger::Stream;

#[derive(Debug)]
pub enum ActionStore {
    /// Hold a sign-in code against an address, replacing any that stood.
    ///
    /// The hash, never the code. The plaintext goes only to
    /// [`ActionEmail::SendCode`](crate::ActionEmail), which is the one place
    /// that needs it.
    SaveCode {
        call_id: CallId,
        email: String,
        code_sha256: Vec<u8>,
        expires_at: api_common::Timestamp,
    },

    /// Count a wrong guess against the code standing for this address.
    RecordAttempt { call_id: CallId, email: String },

    /// Spend the code. What makes a replayed one worth nothing.
    DeleteCode { call_id: CallId, email: String },

    /// Sign somebody in. The runtime resolves the address to a user, which is
    /// a lookup rather than a decision — the machine has no use for a uuid.
    CreateSession {
        call_id: CallId,
        email: String,
        token_sha256: Vec<u8>,
        expires_at: api_common::Timestamp,
    },

    /// Drop every session that has already expired.
    ///
    /// Housekeeping rather than correctness: an expired session signs nobody
    /// in either way, because the lookup filters on expiry. Without this the
    /// table only ever grows (#24).
    ///
    /// Carries the clock, like everything else the machine decided from, so
    /// the row a sweep drops is the one the machine reckoned was expired
    /// rather than one a runtime read a moment later.
    SweepSessions {
        call_id: CallId,
        now: api_common::Timestamp,
    },

    /// Remember that this user opened this Link session.
    ///
    /// Written before the URL reaches them, so there is no window in which a
    /// session could be finished that we have no record of asking for — which
    /// is what stops one account collecting another's bank.
    SaveLink {
        call_id: CallId,
        link_token: String,
        user_id: uuid::Uuid,
    },

    /// Mark a Link session done, whatever it came to. Set on an empty one too:
    /// a session that stays open forever is one we would poll forever.
    CollectLink { call_id: CallId, link_token: String },

    /// Store a newly linked bank. The access token is the runtime's and is not
    /// carried here.
    SaveItem {
        call_id: CallId,
        item_id: String,
        link_token: String,
    },

    /// Write what a sync just produced.
    ///
    /// Carries the diff rather than asking the runtime to read it off state,
    /// because by the time this is performed the diff is gone — state holds the
    /// result of applying it. The clone is one per sync, next to a network
    /// call that cost far more.
    SaveFetched {
        call_id: CallId,
        item_id: String,

        /// What the bank is called, so a cold start has a name to draw before
        /// the first fetch comes back.
        institution_name: Option<String>,

        added: Vec<Entry>,
        modified: Vec<Entry>,
        removed: Vec<String>,

        accounts: Vec<Account>,
        streams: Vec<Stream>,

        /// Written last and separately, so a failure earlier in the batch
        /// leaves the cursor where it was and the next sync re-reads rather
        /// than skipping.
        cursor: String,
    },

    /// Remember that a connection needs a person, or that it no longer does.
    ///
    /// Its own action because it outlives the process: a server that restarts
    /// into an `ITEM_LOGIN_REQUIRED` it has forgotten will cheerfully ask Plaid
    /// again, and be told the same thing.
    SaveStatus {
        call_id: CallId,
        item_id: String,
        error_code: Option<String>,
    },
}

impl ActionStore {
    /// Which call this write is.
    ///
    /// Every variant carries one, so the runtime can report the outcome
    /// against it without knowing which write it just did — which is what
    /// lets there be one report rather than one per arm.
    pub fn call_id(&self) -> CallId {
        match self {
            Self::SaveCode { call_id, .. }
            | Self::RecordAttempt { call_id, .. }
            | Self::DeleteCode { call_id, .. }
            | Self::CreateSession { call_id, .. }
            | Self::SaveLink { call_id, .. }
            | Self::CollectLink { call_id, .. }
            | Self::SaveItem { call_id, .. }
            | Self::SaveFetched { call_id, .. }
            | Self::SaveStatus { call_id, .. }
            | Self::SweepSessions { call_id, .. } => *call_id,
        }
    }
}
